---
name: gal-plan
description: "GAL alias for plan. Use /gal-plan [name] to create a plan scaffold through the dispatcher."
---

# /gal-plan

Run the dispatch script as `plan`, then follow the output block exactly.

## Invoke

**Windows:**
`{{GAL_ROOT}}\scripts\gal.ps1 dispatch plan [args]`

**macOS / Linux:**
`{{GAL_ROOT}}/scripts/gal.sh dispatch plan [args]`

Pass any text the user typed after `/gal-plan` as `[args]`.

## Follow the Output

The script outputs a `--- GAL DISPATCH ---` block. Act on it exactly.
