# FreeCAD MCP Quickstart

This document captures the fastest repeatable path to start FreeCAD for AI-driven CAD work in this repository.

It is based on the Windows workflow verified on 2026-04-29 with:

- FreeCAD `1.1.1`
- `freecad-mcp`
- the Robust MCP Bridge workbench installed under the FreeCAD user mod directory
- GAL local MCP ports pinned to `9873` and `9874`

## Goal

Use this flow when you want an AI agent to:

- start or attach to FreeCAD quickly
- verify the MCP bridge is reachable
- generate or edit geometry
- export `.FCStd`, `.STEP`, `.STL`, or preview images

## Known Good Port Mapping

Keep the client and the bridge aligned on these ports:

- FreeCAD XML-RPC: `9873`
- FreeCAD socket: `9874`

The matching local MCP config shape is:

```json
{
  "servers": {
    "freecad": {
      "command": "freecad-mcp",
      "env": {
        "FREECAD_MODE": "xmlrpc",
        "FREECAD_SOCKET_HOST": "localhost",
        "FREECAD_XMLRPC_PORT": "9873",
        "FREECAD_SOCKET_PORT": "9874"
      }
    }
  }
}
```

On this machine, that alignment is represented in `~/.gal/config/mcp.local.json` and propagated by `scripts/Setup-Machine.ps1`.

## Recommended Startup Path

Prefer the FreeCAD GUI path when you want the most reliable agent experience.

Why:

- GUI mode exposes screenshot and view operations
- the bridge was verified reachable on `localhost:9873`
- the active FreeCAD GUI process successfully owned both `9873` and `9874`
- some agent-side wrappers can show stale disconnected state even when the bridge itself is healthy, so direct bridge verification matters

### Step 1 - Ensure machine config is applied

From the repo root on Windows:

```powershell
scripts\Setup-Machine.ps1
```

This makes sure the VS Code MCP configuration reflects the repo-local FreeCAD settings.

### Step 2 - Start FreeCAD with the Robust MCP Bridge available

Use one of these paths.

#### Option A - Preferred: FreeCAD GUI

Open FreeCAD GUI and let the Robust MCP Bridge workbench run in GUI mode.

If you need an explicit executable path on this machine, it is:

```powershell
& 'C:\Program Files\FreeCAD 1.1\bin\freecad.exe'
```

If bridge auto-start is enabled in the workbench preferences, that is usually enough.

#### Option B - Headless automation

If you need a blocking headless bridge, use `blocking_bridge.py`, not `startup_bridge.py`.

```powershell
$env:FREECAD_XMLRPC_PORT='9873'
$env:FREECAD_SOCKET_PORT='9874'
& 'C:\Program Files\FreeCAD 1.1\bin\freecadcmd.exe' `
  'C:\Users\leetz\AppData\Roaming\FreeCAD\v1-1\Mod\freecad-addon-robust-mcp-server\freecad\RobustMCPBridge\freecad_mcp_bridge\blocking_bridge.py'
```

Do not use this combination for headless startup:

```powershell
freecadcmd startup_bridge.py
```

In this environment it entered the GUI-wait path and stalled on `GuiWaiter`, which is the wrong branch for a console-only run.

## Fast Verification Sequence

### Step 3 - Verify the bridge itself, not just the agent wrapper

Run:

```powershell
$env:FREECAD_MODE='xmlrpc'
$env:FREECAD_SOCKET_HOST='localhost'
$env:FREECAD_XMLRPC_PORT='9873'
$env:FREECAD_SOCKET_PORT='9874'
freecad-mcp --check
```

Expected success shape:

```text
Testing connection to FreeCAD (xmlrpc mode)...
  Host: localhost:9873
✓ Connection successful!
  FreeCAD version: 1.1.1
  GUI available: 1
```

If that succeeds, the bridge is healthy even if an AI tool still reports a stale disconnected state.

### Step 4 - If the wrapper still looks disconnected, verify XML-RPC directly

Use a minimal direct probe:

```python
import xmlrpc.client

proxy = xmlrpc.client.ServerProxy('http://localhost:9873', allow_none=True)
print(proxy.ping())
```

Expected result contains `{"pong": True, ...}`.

This is the fastest way to distinguish a real bridge failure from a client-state mismatch.

## Task Execution Pattern

Once the bridge responds, the fastest reliable execution pattern is:

1. Verify `freecad-mcp --check`
2. If needed, verify `proxy.ping()` over XML-RPC
3. Send a single self-contained modeling script through the bridge
4. Save `.FCStd` and export deliverables in the same script
5. Save a preview image when GUI mode is available

This keeps the run atomic and avoids partial state drift.

### Example: send geometry through XML-RPC

```python
import json
import xmlrpc.client

proxy = xmlrpc.client.ServerProxy('http://localhost:9873', allow_none=True)

fc_code = r'''
import FreeCAD
import Part

doc = FreeCAD.newDocument("ExampleDoc")
box = Part.makeBox(10, 20, 5)
obj = doc.addObject("Part::Feature", "Example")
obj.Shape = box
doc.recompute()

_result_ = {"doc_name": doc.Name}
'''

result = proxy.execute(fc_code)
print(json.dumps(result, indent=2))
```

## Export Pattern

When the task is complete, export everything needed for the next tool or user step in one run:

- `.FCStd` for editable native state
- `.STEP` for CAD interchange
- `.STL` for printing or mesh workflows
- `.png` preview when GUI mode is available

This was the exact pattern used for the `M6OnePieceSpanner` run.

## Common Failure Cases

### Port already in use

Symptoms:

- bridge starts and then exits
- log mentions `JSON-RPC port 9874 already in use`

Check the owner:

```powershell
Get-NetTCPConnection -LocalPort 9874 -State Listen | Select-Object LocalAddress, LocalPort, OwningProcess
Get-Process -Id <PID>
```

On the verified run, the owner was the active `freecad.exe` GUI process.

### `freecad-mcp` works but agent tool still says disconnected

Symptoms:

- `freecad-mcp --check` succeeds
- direct XML-RPC `ping()` succeeds
- agent-side wrapper still reports `Ping failed: Idle`

Interpretation:

- the bridge is up
- the problem is client-side state, not FreeCAD itself

Best next action:

- continue via direct XML-RPC execution or restart the MCP client session

### `startup_bridge.py` hangs under `freecadcmd`

Symptoms:

- output mentions `GUI not ready, using GuiWaiter...`
- `QObject::startTimer: Timers can only be used with threads started with QThread`

Best next action:

- stop using `startup_bridge.py` for headless startup
- switch to `blocking_bridge.py`

## Fast Reuse Checklist For Future AI Sessions

Give the agent this sequence:

1. Confirm ports `9873` and `9874`
2. Run `scripts\Setup-Machine.ps1` if MCP config may be stale
3. Start FreeCAD GUI or `blocking_bridge.py`
4. Run `freecad-mcp --check`
5. If needed, run direct XML-RPC `ping()`
6. Execute one self-contained modeling script
7. Export `.FCStd`, `.STEP`, `.STL`, and preview `.png`

## Related Files

- `~/.gal/config/mcp.local.json`
- `scripts/Setup-Machine.ps1`
- `docs/collaborative-tools/blender-mcp.md`
- `M6OnePieceSpanner.FCStd`
- `M6OnePieceSpanner.step`
- `M6OnePieceSpanner.stl`
- `M6OnePieceSpanner.png`
