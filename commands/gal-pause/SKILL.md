---
name: gal-pause
description: "GAL alias for pause. Use /gal-pause to create a workflow handoff checkpoint through the dispatcher."
---

# /gal-pause

Run the dispatch script as `pause`, then follow the output block exactly.

## Invoke

**Windows:**
`C:\Code\Golem-Agents-Legion\scripts\gal.ps1 dispatch pause [args]`

**macOS / Linux:**
`C:\Code\Golem-Agents-Legion/scripts/gal.sh dispatch pause [args]`

Pass any text the user typed after `/gal-pause` as `[args]`.

## Follow the Output

The script outputs a `--- GAL DISPATCH ---` block. Act on it exactly.
