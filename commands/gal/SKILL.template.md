---
name: gal
description: "GAL — workflow control plane. /gal init · /gal status · /gal whats-next · /gal wrap-up · /gal research · or /gal <golem-name>."
---

# /gal

GAL control-plane entry point. Route based on the subcommand provided.

## Runtime Invocation Note

- In Copilot and Gemini command surfaces, this skill appears conceptually as `/gal`.
- In Codex CLI, custom skills are invoked via `/skills` or `$gal`, not `/gal`.
- For Codex explicit invocation, phrase the request like `$gal status` or `$gal init`.

## Command Routing

| Subcommand | What It Answers | Action |
| --- | --- | --- |
| `init` | How do I bootstrap this repo? | Run `gal.ps1 dispatch init [args]` — follow output block |
| `status` | Where are we right now? | Follow the `/gal-status` procedure — do not run the script |
| `whats-next` | What do I do next? | Follow the `/gal-whats-next` procedure — do not run the script |
| `wrap-up` | How do I close this session cleanly? | Follow the `/gal-wrap-up` procedure — do not run the script |
| `research` | I need structured investigation | Run `gal.ps1 dispatch research [args]` — follow output block |
| `<golem-name>` | I want to consult a specific golem | Run `gal.ps1 dispatch <golem-name> [args]` — follow output block |
| *(no args)* | Auto-detect and recommend | Read `.dev/state.md` and follow the `/gal-whats-next` procedure |

**Removed from public surface:** `plan`, `sync`. See `## Legacy Commands` below.

## Invoke (for script-dispatched subcommands)

**Windows:**
`{{GAL_ROOT}}\scripts\gal.ps1 dispatch [args]`

**macOS / Linux:**
`{{GAL_ROOT}}/scripts/gal.sh dispatch [args]`

## Follow the Output

The script outputs a `--- GAL DISPATCH ---` block. Act on it exactly — no inference, no reinterpretation.

| Field | Meaning |
| --- | --- |
| `COMMAND` | Execute this workflow action: `init` / `error` / `suggest` |
| `ROLE` | Adopt this golem. Mutually exclusive with `COMMAND`. |
| `MODE` | `bound` = act with full authority · `consult` = advise only · `utility` = no restrictions |
| `READ` | Read this file before acting (may appear multiple times) |
| `ACTION` | The specific instruction to execute |
| `ON_COMPLETE` | What to do after finishing |

## Non-Script Procedures

For `status`, `whats-next`, and `wrap-up`, do not run the script. Instead, load and follow the corresponding installed skill:

- `status` → load the installed `gal-status` skill procedure
- `whats-next` → load the installed `gal-whats-next` skill procedure
- `wrap-up` → load the installed `gal-wrap-up` skill procedure

## Legacy Commands

| Old Command | Status | Use Instead |
| --- | --- | --- |
| `/gal next` | Legacy alias for `whats-next` | `/gal whats-next` or `/gal-whats-next` |
| `/gal pause` | Legacy alias for `wrap-up` | `/gal wrap-up` or `/gal-wrap-up` |
| `/gal plan` | Removed — planning is specialist work | `/office-hours`, `/autoplan`, `/plan-eng-review` |
| `/gal sync` | Removed from public surface | Adapter generation is internal to `/gal init` |
