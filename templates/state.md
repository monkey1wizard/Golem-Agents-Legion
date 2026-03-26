# State Template

Template for `<repo>/.dev/state.md` — global index and session continuity.

Per-task state (workflow step, deviations, test/review results) lives in the **plan file's `## Status`** section, not here. This file tracks repo-level concerns only.

## File Template

```markdown
# Project State

## Active Plans

| Plan | Branch | Tier | Workflow State | Last Activity |
| --- | --- | --- | --- | --- |
| [plan path] | [branch] | [T0/T1/T2] | [PLAN/IMPLEMENT/TEST/...] | [YYYY-MM-DD] |

## Global Decisions

| Date | Decision | Rationale | Scope |
| --- | --- | --- | --- |
| YYYY-MM-DD | [Choice] | [Why] | [repo-wide / cross-plan] |

## Blockers

[Repo-level issues preventing progress — empty when unblocked]

## Session Continuity

Last session: YYYY-MM-DD HH:MM
Stopped at: [description of last completed action]
Next step: [what to do when resuming]
Context: [which plan was active, key state to restore]
```

## Usage Rules

1. **Global index, not per-task tracker** — plan files carry their own state via `## Status`
2. **Active Plans table** — add a row when a plan is created, remove when plan is ABSORBED and deleted
3. **Session Continuity** — update at the end of every session or on `gal pause`
4. **Global Decisions** — only for decisions that span multiple plans or affect the entire repo
5. **Keep it minimal** — stale state is worse than no state; don't duplicate what's in plan files
