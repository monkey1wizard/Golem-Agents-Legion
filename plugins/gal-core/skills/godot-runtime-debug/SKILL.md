---
name: godot-runtime-debug
description: Runtime debugging and live inspection for Godot 4 projects. Use when querying the active scene tree, checking signals, reading logs, changing runtime properties, taking screenshots, profiling live behavior, or debugging running Godot C# games.
mcpDependencies:
  optional:
    - godot-mcp
    - godot4-runtime-mcp
license: Complete terms in LICENSE.txt
---

# Godot Runtime Debug

Use this skill when the game is already running or when the problem only appears at runtime.

## Preferred Tool Order

1. Use `godot-mcp` to launch the project and capture debug output.
2. Use `godot4-runtime-mcp` to inspect live nodes, signals, properties, logs, and screenshots.
3. Use Godot CLI DAP options only when you need debugger integration outside MCP.

## Recommended Flows

### Signal Bug

1. Query node signals
2. Inspect existing connections
3. Start signal monitoring
4. Reproduce the bug
5. Read signal event history

### Scene Tree Bug

1. Get the simplified scene tree
2. Search nodes by name or type
3. Inspect the target node context
4. Read or change a runtime property

### Performance Bug

1. Capture baseline logs or a custom marker
2. Read performance stats
3. Filter logs for warnings or errors
4. Take a screenshot if the bug has visible symptoms

## Decision Tree

```text
Need to run or stop the game?
    -> godot-mcp

Need live node, signal, property, or log inspection?
    -> godot4-runtime-mcp

Need CLI debugger attachment for a deeper session?
    -> Godot CLI with --dap-port
```

## Output Requirements

When using this skill:

1. Prefer the smallest live query that can prove or disprove a hypothesis.
2. Capture logs and scene-tree evidence before changing runtime state.
3. Keep runtime edits targeted so they remain debuggable and reversible.