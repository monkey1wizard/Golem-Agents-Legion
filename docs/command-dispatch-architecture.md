# Command Dispatch Architecture

This document explains the durable command-surface model behind GAL.

## Canonical Rule

`/gal` is the canonical command surface.

Everything else exists to improve discoverability, not to create competing execution paths.

## Why Aliases Still Exist

GAL intentionally keeps a small `gal-*` alias set:

- `/gal-init`
- `/gal-plan`
- `/gal-status`
- `/gal-next`
- `/gal-pause`

These aliases exist because slash-command UIs often discover commands through prefix autocomplete. They are UX affordances, not separate architectures.

The important invariant is: all command logic still flows through the dispatcher.

## Dispatcher Contract

`gal dispatch` emits a structured block that the AI should follow deterministically.

```text
--- GAL DISPATCH ---
COMMAND: <init|plan|status|next|pause|error|suggest>
ROLE: <golem-name>
MODE: <bound|consult|utility>
READ: <file-path>
ACTION: <instruction text>
ON_COMPLETE: <next-step hint>
--- END DISPATCH ---
```

Rules:

- `COMMAND` and `ROLE` are mutually exclusive.
- `MODE` is required when `ROLE` is present.
- `READ` is optional and may appear multiple times.
- The AI should execute the block, not reinterpret it into a separate workflow.

## State-Aware Routing

The dispatcher decides between three execution modes.

| Mode | Meaning |
| --- | --- |
| `bound` | The requested golem matches the current workflow state and is being activated in-state |
| `consult` | The golem is being consulted without advancing workflow state |
| `utility` | The golem is independent of workflow state |

## Direct Golem Invocation Policy

Explicit golem targeting is valid, but it does not bypass workflow gates.

| Golem Class | Direct Invocation Policy |
| --- | --- |
| Domain | Allowed as consult |
| Utility | Allowed at any time |
| Workflow | Allowed only as consult unless dispatcher binds it to the current workflow state |

This is the rule that keeps these two statements compatible:

- explicit workflow golem targeting is supported
- state transitions still belong to workflow control, not arbitrary direct calls

In practice, `/gal golem-tester` may be valid as consultation, but it does not replace entering the TEST state through the workflow.

## Reviewer State Compatibility

Current canonical docs use `REVIEW` as the reviewer state.
The dispatcher also accepts legacy `CROSS_REVIEW` values for backward compatibility while routing both to `golem-reviewer`.

New docs and new state files should use `REVIEW`.

## Public Command Surface

Use these as the stable user-facing entry points:

- `/gal`
- `/gal init`
- `/gal plan`
- `/gal status`
- `/gal next`
- `/gal pause`
- `/gal sync`

`/gal sync` is the manual-first adapter generation command. It reads `.dev/project.md`, validates `## Active Skills`, and writes `.github/copilot-instructions.md` plus `GEMINI.md` without mutating any global user configuration.

## Source Of Truth

This document owns the architectural rationale.
These files own the current operational implementation:

- [commands/commands.md](../commands/commands.md)
- [scripts/gal.ps1](../scripts/gal.ps1)
- [scripts/gal.sh](../scripts/gal.sh)
- [scripts/scripts.md](../scripts/scripts.md)
