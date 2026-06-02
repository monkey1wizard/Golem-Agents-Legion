# Godot Module

One-stop reference for the Godot C# toolchain setup and the index of official documentation.

## Toolchain Overview

| Tool | MCP / Runtime | Purpose |
| --- | --- | --- |
| Godot CLI | built-in | build, import, export, CI automation |
| `godot-mcp` | `puntogris/godot-mcp` | launch the editor, run the project, capture debug output |
| `better-godot-mcp` | `kevinwallace/better-godot-mcp` | edit scenes / resources directly without launching the editor |
| `godot4-runtime-mcp` | `AarushShintre/godot4-runtime-mcp` | inspect live nodes, signals, logs, and runtime state |
| VS Code + Godot Extension | — | edit C# gameplay code, browse the SceneTree |

## Choosing a Tool

Use each tool by responsibility:

- **build, import, export, CI automation** → Godot CLI
- **launch the editor, run the project, capture debug output** → `godot-mcp`
- **edit scenes / resources directly without launching the editor** → `better-godot-mcp`
- **inspect live nodes, signals, logs, runtime state** → `godot4-runtime-mcp`

## Godot CLI Examples

```bash
godot --headless --path <project> --build-solutions
godot --headless --path <project> --import
godot --headless --path <project> --export-release <preset> <output>
godot --path <project> -e
godot --path <project>
```

## Key Constraint: Runtime Version Separation

Godot game code and GAL's external tools have different compatibility targets:

- **Godot runtime code** — should stay on the version the project actually supports, usually `.NET 8 / C# 12`.
- **External tools and MCP servers** — may use a newer runtime, because Godot does not load them.

When writing gameplay code, defer to the Godot runtime section in `conventions/csharp.md`.

## Detecting a Godot Project

When initializing `.dev/project.md`, the following signals are strong evidence of a Godot codebase:

- `project.godot` exists at the repo root or app root
- one or more `*.csproj` files alongside or nested for C# gameplay code
- `export_presets.cfg`, `.tscn`, `.tres`, `.res`, or `addons/` appear in the same project tree

## Official Documentation Index

These are the official documentation sources GAL should trust first.

| Topic | Source |
| --- | --- |
| Godot C# basics | [Godot C# Basics](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_basics.html) |
| C# API differences | [Godot C# API Differences](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_differences.html) |
| C# exports | [Godot C# Exports](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_exports.html) |
| C# global classes | [Godot C# Global Classes](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_global_classes.html) |
| C# signals | [Godot C# Signals](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_signals.html) |
| C# Variant | [Godot C# Variant](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_variant.html) |
| C# collections | [Godot C# Collections](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_collections.html) |
| C# style guide | [Godot C# Style Guide](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_style_guide.html) |

### Authority Order

1. Official tool documentation
2. Official API or command-line reference
3. MCP server documentation (to understand the wrapper's capability boundary)
4. Community tutorials (only when official documentation is missing)

## Related Documents

- [README](../../README.md) — what GAL is
- [graphics-workflow](graphics-workflow.md) — the AI-first game art guide
