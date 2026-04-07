# Graphics MCP Setup

This document defines the recommended MCP stack for AI-first game asset production in GAL.

## Preferred Tool Order

1. Use ComfyUI as the generation layer for all asset lanes.
2. Use the lane-specific tool after generation:
    - GIMP for raster cleanup
    - Aseprite for sprite and pixel assets
    - Figma and Inkscape for UI and vector assets
    - Blender for 3D assets

## Stack Summary

| Lane | Tool | MCP / Runtime | Notes |
| --- | --- | --- | --- |
| Generation | ComfyUI | `joenorton/comfyui-mcp-server` | Default local generation server |
| 2D cleanup | GIMP | `maorcc/gimp-mcp` | GIMP 3.0 API bridge |
| Pixel assets | Aseprite | `willibrandon/pixel-mcp` | Sprite, animation, and spritesheet workflow |
| UI layout | Figma | `grab/cursor-talk-to-figma-mcp` | UI and component workflow |
| Vector cleanup | Inkscape | `grumpydevorg/inkscape-mcps` | CLI plus DOM operations |
| 3D general | Blender | `ahujasid/blender-mcp` | Default 3D workflow |

## Decision Tree

```text
Need to generate asset directions or controlled variations?
    -> ComfyUI

Need raster cleanup or compositing?
    -> GIMP MCP

Need pixel production work or spritesheets?
    -> pixel-mcp

Need layout-ready UI or HUD composition?
    -> Figma MCP

Need deterministic SVG cleanup or export?
    -> Inkscape MCP

Need general-purpose 3D modeling and preview renders?
    -> Blender MCP
```

## ComfyUI Setup

Recommended server: `joenorton/comfyui-mcp-server`

### ComfyUI Prerequisites

- Local ComfyUI installation
- Python 3.8+
- ComfyUI running on port `8188` by default

### ComfyUI Install

```bash
git clone https://github.com/joenorton/comfyui-mcp-server.git
cd comfyui-mcp-server
pip install -r requirements.txt
python server.py
```

The MCP endpoint is exposed at `http://127.0.0.1:9000/mcp`.

### ComfyUI Client Config Example

```json
{
  "mcpServers": {
    "comfyui-mcp-server": {
      "type": "streamable-http",
      "url": "http://127.0.0.1:9000/mcp"
    }
  }
}
```

### ComfyUI Use It For

- `generate_image`
- `regenerate`
- `view_image`
- `list_workflows`
- `run_workflow`

## GIMP Setup

Recommended server: `maorcc/gimp-mcp`

### GIMP Prerequisites

- GIMP 3.0+
- Python 3.8+
- `uv`

### GIMP Install

```bash
git clone https://github.com/maorcc/gimp-mcp.git
cd gimp-mcp
uv sync
```

Copy `gimp-mcp-plugin.py` into the GIMP 3.0 plug-ins directory, restart GIMP, then start the MCP server from GIMP via `Tools > Start MCP Server`.

### GIMP Use It For

- full image export for AI review
- metadata inspection
- direct GIMP API calls through `call_api`
- cleanup, compositing, and final raster export

## Aseprite Setup

Recommended server: `willibrandon/pixel-mcp`

### Aseprite Prerequisites

- Aseprite 1.3+
- Go 1.23+ when building from source

### Aseprite Install

```bash
git clone https://github.com/willibrandon/pixel-mcp.git
cd pixel-mcp
make build
```

Create the config file under `~/.config/pixel-mcp/config.json` and point `aseprite_path` at the local Aseprite binary.

### Aseprite Use It For

- canvas and layer management
- palette operations
- dithering, shading, and antialiasing
- animation frames and tags
- spritesheet export

## Figma Setup

Recommended server: `grab/cursor-talk-to-figma-mcp`

### Figma Prerequisites

- Figma account and API or plugin-side setup as required by the server
- MCP client support for the server's transport mode

### Figma Use It For

- component structure
- auto layout and state variants
- design-system-oriented UI asset work
- export handoff for UI and HUD assets

## Inkscape Setup

Recommended server: `grumpydevorg/inkscape-mcps`

### Inkscape Prerequisites

- Python 3.9+
- Inkscape installed and available in `PATH`

### Inkscape Install

```bash
pip install inkscape-mcp
```

Or clone the repo and install from source.

### Inkscape Client Config Example

```json
{
  "mcpServers": {
    "inkscape-mcp": {
      "command": "inkscape-mcp",
      "env": {
        "INKS_WORKSPACE": "/path/to/workspace"
      }
    }
  }
}
```

### Inkscape Use It For

- path operations
- SVG validation and cleanup
- export to PNG, PDF, or plain SVG
- DOM-level deterministic edits

## Blender Setup

Recommended server: `ahujasid/blender-mcp`

### Blender Use It For

- object and material operations
- scene changes
- screenshots and quick iteration
- AI-assisted Blender-side automation

### Blender CLI Companion

Use Blender CLI when deterministic batch work is easier than MCP interaction.

```bash
blender -b scene.blend -P script.py
```

## Deferred Options

- `AIDC-AI/Pixelle-MCP` is deferred for now. Revisit it when cloud execution or no-GPU generation becomes a first-class need.
- Krita is deferred from MCP integration and can be used through CLI-only export when needed.

## Output Requirements

When setting up this stack:

1. Install only the MCP servers required for the lanes you actually use.
2. Keep ComfyUI available first; all other tools depend on generated or refined assets.
3. Keep Blender as the default and only documented 3D lane tool in this stack.
