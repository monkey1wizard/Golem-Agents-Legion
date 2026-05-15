---
name: gal
description: "GAL — workflow control plane. /gal init · /gal status · /gal whats-next · /gal wrap-up · /gal research · /gal deep-research · or /gal <golem-name>."
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
| `deep-research` | I need multi-source investigation with cross-review | Run `gal.ps1 dispatch deep-research [args]` — follow output block |
| `pipeline` | Task-driven autopilot: iterate T-NNN tasks with implement → commit → test → review per task, final verifier pass; stop only on human-required blockers, retry ceiling, working-hours boundary, or `stop-at` | Run `gal.ps1 dispatch pipeline` — follow output block |
| `<golem-name>` | I want to consult a specific golem | Run `gal.ps1 dispatch <golem-name> [args]` — follow output block |
| *(no args)* | Auto-detect and recommend | Locate the nearest ancestor repo root containing `.dev/state.md`, then follow the `/gal-whats-next` procedure |

## xmachine Activation Rule

Remote execution intent is active only when the request contains both:

- the literal keyword `xmachine`
- a valid node alias from `xmachine.config.json`

If either one is missing, do not infer xmachine routing.

Accepted examples:

- `/gal pipeline --xmachine node-name`
- `/gal research use xmachine node-name`
- `/gal golem-researcher use xmachine node-name for bounded execution`

Rejected examples:

- `/gal pipeline on node-name`
- `/gal research xmachine`
- `/gal pipeline --xmachine`

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

Before invoking a dispatcher, choose the target project root as follows:

- For `init`, treat the current working directory as the target project root unless the user explicitly provided another target path. Fresh repos often do not have `.dev/state.md` yet.
- For other script-dispatched subcommands, walk upward from the current working directory or provided `#file:` path until `.dev/state.md` is found.

Keep the terminal current directory at that target project root so dispatcher state reads and plan paths resolve against the project being worked on.

**Windows:**

1. If `.\scripts\gal.ps1` exists in the target project, run `.\scripts\gal.ps1 dispatch [args]`.
2. Otherwise run the GAL runtime checkout's `scripts/gal.ps1 dispatch [args]` while staying in the target project root.

**macOS / Linux:**

1. If `./scripts/gal.sh` exists in the target project, run `./scripts/gal.sh dispatch [args]`.
2. Otherwise run the GAL runtime checkout's `scripts/gal.sh dispatch [args]` while staying in the target project root.

The fallback runtime path is expected for two cases:

- fresh repos being bootstrapped with `init`, before local `scripts/` or `.dev/state.md` exist
- initialized plan-only projects that have `.dev/state.md` but do not contain GAL's `scripts/` directory

In that mode, xmachine node aliases are validated from the GAL runtime checkout's `xmachine.config.json`, while `.dev/state.md` and plan files are still read from the target project root.

## Follow the Output

The script outputs a `--- GAL DISPATCH ---` block. Act on it exactly — no inference, no reinterpretation.

| Field | Meaning |
| --- | --- |
| `COMMAND` | Execute this workflow action: `init` / `error` / `suggest` |
| `ROLE` | Adopt this golem. Mutually exclusive with `COMMAND`. |
| `MODE` | `bound` = act with full authority · `consult` = advise only · `utility` = no restrictions |
| `READ` | Read this file before acting (may appear multiple times) |
| `PLAN` | Optional explicit plan file path for workflows that support file override. When present, prefer this plan over `.dev/state.md` active-plan lookup. |
| `EXECUTION` | Optional execution hint. `xmachine` means the dispatcher has validated the activation phrase for remote execution. |
| `WORK_NODE` | Optional xmachine node alias from `xmachine.config.json`. Present only when `EXECUTION: xmachine` is emitted. |
| `ACTION` | The specific instruction to execute |
| `ON_COMPLETE` | What to do after finishing |

## Non-Script Procedures

For `status`, `whats-next`, and `wrap-up`, do not run the script. Instead, load and follow the corresponding installed skill:

- `status` → load the installed `gal-status` skill procedure
- `whats-next` → load the installed `gal-whats-next` skill procedure
- `wrap-up` → load the installed `gal-wrap-up` skill procedure

Treat those delegated skill procedures as the single source of truth for substantive control-plane behavior.
