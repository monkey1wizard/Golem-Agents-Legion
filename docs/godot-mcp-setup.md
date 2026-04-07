# Godot C# MCP Setup

This document defines the default GAL tool stack for Godot 4 C# work.

## Recommended Stack

| Layer | Tool | Purpose |
| --- | --- | --- |
| Editor and run control | [Coding-Solo/godot-mcp](https://github.com/Coding-Solo/godot-mcp) | Launch the editor, run projects, inspect debug output, and manage scenes |
| Offline scene authoring | [n24q02m/better-godot-mcp](https://github.com/n24q02m/better-godot-mcp) | Modify `.tscn` and other Godot resources without booting the editor |
| Runtime inspection | [MingHuiLiu/godot4-runtime-mcp](https://github.com/MingHuiLiu/godot4-runtime-mcp) | Query the live scene tree, signals, properties, logs, screenshots, and execute small C# snippets |
| Build and export | [Godot CLI](https://docs.godotengine.org/en/stable/tutorials/editor/command_line_tutorial.html) | Build C# solutions, import assets, export builds, and run headless automation |
| C# code intelligence | VS Code C# tooling | IntelliSense, rename, references, diagnostics, and refactoring |

## Why This Stack

- `godot-mcp` is the highest-signal general-purpose Godot MCP currently available and handles the editor control loop well.
- `better-godot-mcp` covers deterministic file-level authoring without requiring a running Godot instance.
- `godot4-runtime-mcp` covers the part the others do not: live runtime state, signal debugging, and in-process inspection for C# projects.
- Godot already ships the CLI commands needed for build, import, and export, so a separate C#-only build MCP is not required.

## Default Tool Order

1. Use Godot CLI for build, import, export, and scripted batch work.
2. Use `godot-mcp` when you need to launch or stop the editor, run the game, or capture debug output.
3. Use `better-godot-mcp` when you need deterministic offline scene or resource edits.
4. Use `godot4-runtime-mcp` when the game is already running and you need to inspect the live world.

## Godot CLI Commands

| Task | Command |
| --- | --- |
| Build C# gameplay code | `godot --headless --path <project> --build-solutions` |
| Import assets | `godot --headless --path <project> --import` |
| Export release build | `godot --headless --path <project> --export-release <preset> <output>` |
| Export debug build | `godot --headless --path <project> --export-debug <preset> <output>` |
| Run the editor | `godot --path <project> -e` |
| Run the project | `godot --path <project>` |
| Run a utility script | `godot --headless --path <project> -s <script.gd>` |
| Enable debug adapter | `godot --path <project> --dap-port <port>` |

## Installation Notes

### Coding-Solo/godot-mcp

- Install with `npx @coding-solo/godot-mcp`
- Set `GODOT_PATH` when auto-detection is unreliable
- Best for launch, run, stop, debug output, and scene management workflows

### better-godot-mcp

- Install with `npx @n24q02m/better-godot-mcp`
- Supports local or Docker-based use
- Best for `.tscn`, shader, tilemap, audio, navigation, and UI edits without opening the editor

### godot4-runtime-mcp

- Build the MCP server with `dotnet build`
- Add the provided Godot plugin file to the project and register it as an AutoLoad
- Start the game so the HTTP bridge is available before using runtime tools

## Decision Tree

```text
Need to build, import, or export?
    -> Godot CLI

Need to launch the editor, run the game, or collect debug output?
    -> godot-mcp

Need to create or modify scenes/resources without a running editor?
    -> better-godot-mcp

Need to inspect live nodes, signals, logs, or runtime properties?
    -> godot4-runtime-mcp
```

## Optional Premium Alternative

[youichi-uda/godot-mcp-pro](https://github.com/youichi-uda/godot-mcp-pro) is a larger paid toolset. It is not the default recommendation, but it can be evaluated if the free stack leaves a clear capability gap.
