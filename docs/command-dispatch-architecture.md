# Command Dispatch Architecture

This document explains the durable command-surface model behind GAL.

## Canonical Rule

`/gal` is the canonical control-plane entry point.

Everything else exists to improve discoverability or to provide substantive procedures for specific operations, not to create competing execution paths.

## Script Design Contract

`scripts/gal.ps1` and `scripts/gal.sh` are **internal routing helpers**, not user-facing command surfaces.

Rules:

- Scripts are invoked by the AI after reading a `--- GAL DISPATCH ---` block, not typed directly by users.
- The only valid top-level commands are `init` and `dispatch`. Any other direct invocation exits with a migration error.
- `dispatch` accepts two subcommands: `init` (bootstraps a repo) and `research` (routes to the research skill). All other `dispatch` arguments are golem names.
- `/gal status`, `/gal whats-next`, and `/gal wrap-up` are **substantive skills** — they have their own SKILL.md files and do not route through the script at all.
- Internal adapter generation is plumbing inside `gal init`, not a separate public command.

## Two Kinds of Alias Skills

GAL `gal-*` alias skills come in two kinds:

**Substantive skills** — contain full procedures and do not dispatch through the script:

- `/gal-status` — full state projection (reads `.dev/state.md` and active plan files)
- `/gal-whats-next` — next-action recommendation (reads state, applies decision tree)
- `/gal-wrap-up` — session close-out (writes `### Handoff Notes` and `## Session Continuity`)

**Script-forwarding aliases** — exist only for autocomplete discoverability and route through `gal dispatch`:

- `/gal-init`

## Public Command Surface

Use these as the stable user-facing entry points:

| Command | What It Answers |
| --- | --- |
| `/gal init` | How do I bootstrap this repo? |
| `/gal status` | Where are we right now? |
| `/gal whats-next` | What do I do next? |
| `/gal wrap-up` | How do I close this session cleanly? |
| `/gal research` | I need structured investigation |

## Dispatcher Contract

`gal dispatch` emits a structured block for script-dispatched subcommands (`init`, `research`, `pipeline`, golem names).

```text
--- GAL DISPATCH ---
COMMAND: <init|error|suggest>
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

## Invocation Modes

The dispatcher uses lightweight invocation modes only.

| Mode | Meaning |
| --- | --- |
| `consult` | The golem is being consulted without any dispatcher-owned state transition |
| `utility` | The golem is an independent helper |

## Direct Golem Invocation Policy

Explicit golem targeting is valid, but it does not replace specialist command procedures.

| Golem Class | Direct Invocation Policy |
| --- | --- |
| Domain | Allowed as consult |
| Utility | Allowed at any time |
| Pipeline | Allowed as consult; full execution authority comes from `/gal pipeline` or another specialist workflow |

## Source Of Truth

This document owns the architectural rationale.
These files own the current operational implementation:

- [commands/commands.md](../commands/commands.md)
- [docs/gal-control-plane-contracts.md](gal-control-plane-contracts.md)
- [scripts/gal.ps1](../scripts/gal.ps1)
- [scripts/gal.sh](../scripts/gal.sh)
- [scripts/scripts.md](../scripts/scripts.md)
