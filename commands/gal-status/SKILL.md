---
name: gal-status
description: "GAL alias for status. Use /gal-status to inspect the current workflow state through the dispatcher."
---

# /gal-status

Run the dispatch script as `status`, then follow the output block exactly.

## Invoke

**Windows:**
`C:\Code\Golem-Agents-Legion\scripts\gal.ps1 dispatch status [args]`

**macOS / Linux:**
`C:\Code\Golem-Agents-Legion/scripts/gal.sh dispatch status [args]`

Pass any text the user typed after `/gal-status` as `[args]`.

## Follow the Output

The script outputs a `--- GAL DISPATCH ---` block. Act on it exactly.
