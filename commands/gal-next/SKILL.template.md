---
name: gal-next
description: "GAL alias for next. Use /gal-next to resume the next recorded workflow step through the dispatcher."
---

# /gal-next

Run the dispatch script as `next`, then follow the output block exactly.

## Invoke

**Windows:**
`{{GAL_ROOT}}\scripts\gal.ps1 dispatch next [args]`

**macOS / Linux:**
`{{GAL_ROOT}}/scripts/gal.sh dispatch next [args]`

Pass any text the user typed after `/gal-next` as `[args]`.

## Follow the Output

The script outputs a `--- GAL DISPATCH ---` block. Act on it exactly.
