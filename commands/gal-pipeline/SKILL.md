---
name: gal-pipeline
description: "Auto-chain pipeline. Runs implement → test → review in sequence using model-roles for multi-vendor AI assignment. Each phase uses a different AI vendor per model-roles.local.md."
---

# /gal-pipeline

Run the full implementation pipeline in one command: implement → test → review. Each phase is dispatched to a different AI vendor per `model-roles.local.md`.

## Role

Pipeline orchestrator. Your job is to chain three golem phases in sequence, advancing only when each phase succeeds, and stopping to surface blockers when they arise.

## When to Use

- After `/autoplan` (or `/plan-eng-review`) has produced a `## Test Plan`
- When you want full implement → test → review without manual intervention
- When the user says "start implementation", "run the pipeline", "implement and test", or similar

## Model Assignment

Each phase uses a different AI vendor, enforced by `model-roles.local.md`:

| Phase | Golem | Role | Why different |
| --- | --- | --- | --- |
| Implement | `golem-implementer` | CODER | Writes the code |
| Test | `golem-tester` | TESTER | Must not read implementation — writes tests from spec only |
| Review | `golem-reviewer` | REVIEWER | Must differ from CODER — fresh eyes on bugs and architecture |

## Step 1 — Read Plan and Verify Prerequisites

Read the active plan file from `.dev/state.md`.

Verify:
- `## Test Plan` exists in the plan file (required for golem-tester)
- Workflow state is `IMPLEMENT` or earlier (do not re-run if already `REVIEW`-complete)
- No unresolved `BLOCKING` items in `## Review Results`

If prerequisites are not met: tell the user what is missing and stop.

## Step 2 — Implement (CODER model)

Run:
```
C:\Code\Golem-Agents-Legion\scripts\gal.ps1 dispatch golem-implementer
```

Follow the dispatch block exactly. Adopt `golem-implementer` in `bound` mode and execute the plan.

When implementation is complete:
- Confirm `## Status` in the plan shows `Workflow: IMPLEMENT` with all steps done
- Update plan `## Status`: set `Workflow: TEST`

## Step 3 — Test (TESTER model — different vendor from CODER)

Run:
```
C:\Code\Golem-Agents-Legion\scripts\gal.ps1 dispatch golem-tester
```

Follow the dispatch block exactly. Adopt `golem-tester` in `bound` mode.

`golem-tester` reads only the plan spec and public API — never the implementation code.

When testing is complete, check `## Test Results`:
- If **all tests pass**: update plan `## Status`: set `Workflow: REVIEW` — proceed to Step 4
- If **any tests FAIL**: surface the failures to the user and **stop**. Do not proceed to review. Tell the user to fix the failures and re-run `/gal pipeline` or `/gal golem-implementer`.

## Step 4 — Review (REVIEWER model — different vendor from CODER and TESTER)

Run:
```
C:\Code\Golem-Agents-Legion\scripts\gal.ps1 dispatch golem-reviewer
```

Follow the dispatch block exactly. Adopt `golem-reviewer` in `bound` mode.

When review is complete, check `## Review Results`:
- If **BLOCKING findings exist**: surface them to the user and **stop**. List each `[B-NN]` item with its suggested fix.
- If **WARNING or INFO only (no BLOCKING)**: proceed to Step 5.

## Step 5 — Final Gate

Report the combined verdict to the user:

```
--- PIPELINE COMPLETE ---

Implement:  ✓ done  (<N> commits)
Test:       ✓ <N> passed / <N> failed
Review:     ✓ APPROVE  (or ✗ REQUEST_CHANGES — <N> BLOCKING)

Overall: READY FOR SHIP  (or: BLOCKED — see above)
```

If overall READY: tell the user to run `/ship` as the next step.
If overall BLOCKED: list the blockers clearly and stop.

## Non-Script Fallback

If the script cannot be run (e.g. macOS / Linux), run:
```
C:\Code\Golem-Agents-Legion/scripts/gal.sh dispatch golem-implementer
```
(and equivalent for tester and reviewer)

Or invoke each golem directly by asking the user to switch to the appropriate AI model and following the respective agent file:
- `agent/golem-implementer.agent.md`
- `agent/golem-tester.agent.md`
- `agent/golem-reviewer.agent.md`
