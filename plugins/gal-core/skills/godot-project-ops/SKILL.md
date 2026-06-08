---
name: godot-project-ops
description: Project scaffolding, build, import, export, and headless automation for Godot 4 C# projects. Use when creating a Godot project, building C# gameplay code, importing assets, exporting builds, or choosing between Godot CLI and Godot MCP tools.
cliDependencies:
    required:
        - godot
mcpDependencies:
  optional:
    - godot-mcp
    - better-godot-mcp
license: Complete terms in LICENSE.txt
---

# Godot Project Ops

Use this skill for top-level project operations in Godot 4 C# repos.

## Preferred Tool Order

1. Use Godot CLI for build, import, export, and batch automation.
2. Use `godot-mcp` when you need to launch the editor, run the project, or inspect debug output.
3. Use `better-godot-mcp` when you need file-level project changes without a running editor.

## Availability Check

Before relying on the CLI path, verify Godot is available:

```bash
godot --version
```

- Exit code `0`: Godot CLI is available.
- Non-zero or command-not-found: use the MCP path only for tasks it can actually cover.

## Common Tasks

| Task | Preferred Path |
| --- | --- |
| Create project skeleton | Filesystem plus `project.godot` and `.csproj` setup |
| Build gameplay code | `godot --headless --path <project> --build-solutions` |
| Import assets | `godot --headless --path <project> --import` |
| Export build | `godot --headless --path <project> --export-release <preset> <output>` |
| Launch editor | `godot --path <project> -e` or `godot-mcp` editor launch |
| Run project | `godot --path <project>` or `godot-mcp` project run |

## Decision Tree

```text
Need build, import, export, or CI automation?
    -> Godot CLI

Need to open the editor or control a run loop?
    -> godot-mcp

Need deterministic file changes without booting Godot?
    -> better-godot-mcp
```

## Output Requirements

When using this skill:

1. Prefer Godot CLI for reproducible project operations.
2. Treat Godot runtime code and external tooling as separate .NET compatibility zones.
3. Verify `project.godot` and `*.csproj` exist before assuming a Godot C# repo layout.

## No-Tool Behavior

If the requested task depends on build, import, export, or other CLI-owned operations and Godot CLI is unavailable, stop and report that the required Godot command path is missing.

- Use MCP tools only for the editor, run-loop, or file-level tasks they actually support.
- Do not pretend MCP can replace headless build or export operations when it cannot.
- If both the CLI and the needed MCP capability are unavailable, report the blocker explicitly.