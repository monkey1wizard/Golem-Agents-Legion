---
name: gal-whats-next
description: "GAL — what to do next. Reads current plan state, review and test results, and recommends the single next specialist command or control-plane action."
---

# /gal whats-next

Determine what to do right now based on the current recorded GAL state.

## Step 1 — Read State

Read these files in order:

1. `.dev/state.md` — active plans table, blockers, session continuity
2. The active execution plan file from the Active Plans table (`docs/plans/<plan-slug>.prompt.md`) — `## Status` section (workflow state, current step, next step)
3. Active plan `## Review Results` — any BLOCKING findings
4. Active plan `## Test Results` — pass / fail / pending
5. Active plan `### Handoff Notes` — interrupted work context
6. Active plan `## Open Questions` — count of unresolved OQ-NNN items
7. Active plan `## Tasks` — task completion state
8. Active plan `## Analyze` — CLEAR / DRIFT-OPEN / NOT-RUN verdict

If `.dev/state.md` does not exist: output **Repo not initialized — run `/gal init`.**

If `.dev/state.md` exists but the Active Plans table or the active plan `## Status` cannot be read cleanly, output **Repo is initialized, but GAL state is malformed — inspect `.dev/state.md` Active Plans and the active `.prompt.md` file.**

## Step 2 — Decide

Apply this decision tree in order:

| Condition | Next Action |
| --- | --- |
| No active plan, no work in progress | Repo is already initialized; use `/office-hours` to start sprint planning |
| Active plan in DRAFT, no eng review recorded | `/plan-eng-review` to get the plan reviewed before starting work |
| Active plan approved, implementation not started | Describe the first implementation task from the plan |
| Implementation in progress, `### Handoff Notes` present | Resume from the exact "next step" in Handoff Notes |
| Implementation complete, no test results | `/qa` for full QA, or `/qa-only` for focused test run |
| Tests failing | Return to implementation — summarize what needs fixing |
| Tests passing, no review recorded | `/review` for code review |
| Review has BLOCKING findings | Address the BLOCKING items — return to implementation |
| `<!-- ANALYZE: DRIFT-OPEN -->` present | Diff has drifted from plan scope — address deviations, then re-run `/review` to update verdict |
| `## Tasks` has incomplete items and no BLOCKING findings | Return to implementation — list remaining T-NNN tasks |
| Open OQs remain in `## Open Questions` | Note count as advisory — do not block; continue to next step |
| Review clean, plan not yet verified | `/gal wrap-up` to close the session for handoff |
| Blocker listed in `.dev/state.md` | State the blocker and what resolves it before any other action |
| Session continuity shows interrupted work | Resume from "Stopped at" in `.dev/state.md` `## Session Continuity` |

## Step 3 — Output

State in plain language:

1. **Where you are** — one sentence describing the current position in the workflow
2. **Next action** — the single command to run, or the single task to start
3. **Open this first** — which file or context is needed to begin

Do not present multiple options. Commit to one clear next step.
