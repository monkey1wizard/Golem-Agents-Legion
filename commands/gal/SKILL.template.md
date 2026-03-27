---
description: "GAL — workflow dispatcher. Auto-detects state, routes to the right golem, drives the coding workflow. Use /gal <subcommand> or /gal <golem> [text]."
---

# /gal

Run the dispatch script with any arguments the user provided, then follow the output block exactly.

## Invoke

**Windows:**
`{{GAL_ROOT}}\scripts\gal.ps1 dispatch [args]`

**macOS / Linux:**
`{{GAL_ROOT}}/scripts/gal.sh dispatch [args]`

Pass any text the user typed after `/gal` as `[args]`.

## Follow the Output

The script outputs a `--- GAL DISPATCH ---` block. Act on it exactly — no inference, no reinterpretation.

| Field | Meaning |
|---|---|
| `COMMAND` | Execute this workflow action: `init` / `plan` / `status` / `next` / `pause` / `error` / `suggest` |
| `ROLE` | Adopt this golem. Mutually exclusive with `COMMAND`. |
| `MODE` | `bound` = act with full authority · `consult` = advise only · `utility` = no restrictions |
| `READ` | Read this file before acting (may appear multiple times) |
| `ACTION` | The specific instruction to execute |
| `ON_COMPLETE` | What to do after finishing |
