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
| `pipeline` | Task-driven autopilot: iterate T-NNN tasks with implement → commit → test → review per task, final verifier pass; stop only on human-required blockers, retry ceiling, curfew, or `stop-at` | Run `gal.ps1 dispatch pipeline` — follow output block |
| `<golem-name>` | I want to consult a specific golem | Run `gal.ps1 dispatch <golem-name> [args]` — follow output block |
| *(no args)* | Auto-detect and recommend | Locate the nearest ancestor repo root containing `.dev/state.md`, then follow the `/gal-whats-next` procedure |

## Natural Language Pipeline Trigger

If the user's message contains any of the following intents, treat it as `/gal pipeline`:

- "start implementation"
- "implement and test"
- "implement and review"
- "run the pipeline"
- "auto implement"
- "開始實作"
- "開始實作並自動執行"
- "自動執行 review 和 test"

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

Treat those delegated skill procedures as the single source of truth for substantive control-plane behavior.

