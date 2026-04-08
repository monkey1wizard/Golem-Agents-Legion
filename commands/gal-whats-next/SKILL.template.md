---
name: gal-whats-next
description: "GAL — what to do next. Reads current plan state, review and test results, and recommends the single next specialist command or control-plane action."
---

# /gal whats-next

Determine what to do right now based on the current recorded GAL state.

## Step 1 — Collect State

Starting from the current working directory or opened workspace folder, walk upward to the nearest ancestor directory that contains `.dev/state.md`. Treat that ancestor as the repo root and read `.dev/state.md` there.

- If no ancestor directory contains `.dev/state.md`, output **Repo not initialized — run `/gal init`.**
- If `.dev/state.md` exists but there is no active plan entry under `## Active Plans`, output **No active plan. Use `/office-hours` to start sprint planning.**
- If `.dev/state.md` exists and names an active plan, read that plan's execution file from the `File` column. Resolve markdown-wrapped relative paths against the current repo root. If the row points to `docs/plans/<slug>.md`, prefer `docs/plans/<slug>.prompt.md` when it exists.
- If the active plan file is missing or its `## Status` section does not expose a `Workflow:` field, output the exact repo-state error and suggest inspecting `.dev/state.md` plus the referenced active plan file.

From `.dev/state.md` and the active plan file, extract these data points:

1. `.dev/state.md` — active plans table, blockers, session continuity
2. Active plan `## Status` — workflow state, current step, next step
3. Active plan `## Review Results` — any BLOCKING findings
4. Active plan `## Test Results` — pass / fail / pending
5. Active plan `### Handoff Notes` — interrupted work context
6. Active plan `## Open Questions` — count of unresolved OQ-NNN items
7. Active plan `## Tasks` — task completion state
8. Active plan `## Analyze` — CLEAR / DRIFT-OPEN / NOT-RUN verdict

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
