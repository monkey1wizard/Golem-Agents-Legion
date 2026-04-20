---
name: gal-pipeline
description: "Task-driven autopilot. Iterates through every T-NNN task in the active plan running implement → test → review per task, with a mandatory git commit gate between tasks, and a final verifier pass at the end. Stops only on human-required blockers, retry ceiling breach, working-hours boundary, or a user-specified stop boundary."
---

# /gal-pipeline

Run the full implementation pipeline task by task: for each `T-NNN` task in the active plan, run implement → commit → test → review in sequence, then advance to the next task. Each phase uses a different AI vendor per `model-roles.local.md`. After all tasks complete, run a final verifier pass.

## Role

Pipeline orchestrator. Your job is to iterate through plan tasks automatically, advancing only when each task's commit + test + review gate is fully clean, and stopping only when a genuine human-required condition is encountered.

## When to Use

- After the engineering review lane has produced a `## Tasks` section and a `## Test Plan`
- When you want full task-by-task automation without manual intervention
- When the user says "start implementation", "run the pipeline", "implement and test", or similar

## Syntax

```
/gal pipeline [from T-NNN] [stop-at T-NNN]
```

- **No arguments**: start from the first unchecked task, run until all tasks complete
- **`from T-NNN`**: start from the specified task (skip earlier unchecked tasks)
- **`stop-at T-NNN`**: after completing `T-NNN`, stop before starting the next task and prompt the user
- **Resume**: if `Current Task` is set in `## Status`, resume from that task (overridden by explicit `from`)

## Model Assignment

Each phase uses a different AI vendor, enforced by `model-roles.local.md`:

| Phase | Golem | Role | Why different |
| --- | --- | --- | --- |
| Implement | `golem-implementer` | CODER | Writes the code |
| Test | `golem-tester` | TESTER | Must not read implementation — writes tests from spec only |
| Review | `golem-reviewer` | REVIEWER | Must differ from CODER — fresh eyes on bugs and architecture |
| Verify | `golem-verifier` | VERIFIER | Must differ from CODER — goal-backward plan verification |

---

## Step 1 — Read Plan and Verify Prerequisites

Read the active plan file from `.dev/state.md`.

Verify:
- `## Tasks` exists with at least one `T-NNN` task
- `## Test Plan` exists in the plan file (required for golem-tester)
- No unresolved `BLOCKING` items in `## Review Results` at the root level
- Workflow state is not already `DONE`

If `Current Task` is set in `## Status` and no `from` argument was given, resume from that task.

If prerequisites are not met: tell the user what is missing and stop.

---

## Step 2 — Task Loop

Repeat for each unchecked `T-NNN` task (in order, respecting `from` / `stop-at`):

### 2a — Working Hours Check

Before starting a new task, check the current time against the working-hours policy in `conventions/working-hours.md`.

- If the working-hours boundary is active: do not start the next task. Offer `/gal wrap-up` once (do not auto-run it). Wait for explicit user confirmation before proceeding. Stop here.
- If the working-hours boundary is not active: continue.

### 2b — Update Cursor

Update plan `## Status`:
```
Current Task: T-NNN
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0
Workflow: IMPLEMENT
```

### 2c — Implement (CODER model)

Run:
```
C:\Code\Golem-Agents-Legion\scripts\gal.ps1 dispatch golem-implementer
```

Invoke with `TASK_SCOPE: T-NNN`. The implementer must:
1. Record `Task Base Commit` in `## Status` before any changes
2. Implement only the work required by `T-NNN`
3. Record `Task Final Commit` in `## Status` when done
4. Ensure `git status` is clean before reporting complete

**Hard Commit Gate:** If `git status` is not clean or `Task Final Commit` is not recorded, do not proceed. Stop and surface the issue.

### 2d — Test (TESTER model — different vendor from CODER)

Update plan `## Status`: set `Workflow: TEST`

Run:
```
C:\Code\Golem-Agents-Legion\scripts\gal.ps1 dispatch golem-tester
```

Invoke in task-scoped mode for `T-NNN`. The tester writes a `### [T-NNN] YYYY-MM-DD` subsection under `## Test Results`.

Check result:
- **All tests PASS**: update `## Status` `Workflow: REVIEW`, proceed to 2e
- **Any tests FAIL**:
  - Increment `Test Retry Count` in `## Status`
  - If `Test Retry Count` < 3: dispatch implementer to fix failing tests (TASK_SCOPE: T-NNN, fix mode), then re-run tester
  - If `Test Retry Count` = 3: **STOP**. Surface failures. Tell user the retry ceiling (3) has been reached for `T-NNN` and request human intervention

### 2e — Review (REVIEWER model — different vendor from CODER and TESTER)

Run:
```
C:\Code\Golem-Agents-Legion\scripts\gal.ps1 dispatch golem-reviewer
```

Invoke in task-scoped mode for `T-NNN` with commit range `Task Base Commit..Task Final Commit`. The reviewer writes a `### [T-NNN] YYYY-MM-DD` subsection under `## Review Results`.

Check result:
- **APPROVE (no BLOCKING)**: proceed to 2f
- **REQUEST_CHANGES or BLOCK (BLOCKING findings)**:
  - Increment `Review Retry Count` in `## Status`
  - If `Review Retry Count` < 3: dispatch implementer to fix BLOCKING issues (TASK_SCOPE: T-NNN, fix mode), update `Task Final Commit`, then re-run reviewer
  - If `Review Retry Count` = 3: **STOP**. Surface BLOCKING findings. Tell user the retry ceiling (3) has been reached for `T-NNN` and request human intervention

**Security / Protected Path escalation:** If any BLOCKING finding is a security vulnerability or Protected Path violation, **STOP immediately** regardless of retry count. Do not attempt an automated fix. Surface to human.

### 2f — Mark Task Complete

All gates passed for `T-NNN`:
1. Mark `T-NNN` as complete in `## Tasks` (check the checkbox)
2. Update plan `## Status`:
   ```
   Last activity: YYYY-MM-DD — T-NNN complete (commit: <Task Final Commit>)
   ```
1. If `stop-at T-NNN` was specified and this task matches: **STOP**. Report task complete and prompt user before starting the next task.
2. Otherwise: advance to the next unchecked task and return to 2a.

---

## Step 3 — Post-Loop Verifier

After all unchecked tasks are complete, dispatch `golem-verifier` for a plan-level goal-backward verification pass.

**IMPORTANT:** Invoke `golem-verifier` for **Steps 1–4 only** (produce a VERIFIED / GAPS_FOUND / BLOCKED verdict). Do **NOT** trigger Step 5 (lifecycle ending: ABSORBED marking + plan file deletion) — that remains a post-release action.

Run:
```
C:\Code\Golem-Agents-Legion\scripts\gal.ps1 dispatch golem-verifier
```

Instruct the verifier explicitly: "Run Steps 1–4 only. Do not mark the plan ABSORBED or delete the plan file."

Check result:
- **VERIFIED**: proceed to Step 4 (final gate)
- **GAPS_FOUND**: **STOP**. Surface each gap with its description. Tell user to resolve the gaps before handing off to `golem-releaser`.
- **BLOCKED**: **STOP**. Surface the blocking condition. Tell user to resolve before release work starts.

---

## Step 4 — Final Gate

Report the combined verdict:

```
--- PIPELINE COMPLETE ---

Tasks completed: N of N
  T-001  ✓ implement · test · review
  T-002  ✓ implement · test · review
  ...

Verifier: VERIFIED

Overall: READY FOR RELEASE
```

Tell the user to route the next step to `golem-releaser`.

If any task or verifier is blocked, report with detail:

```
--- PIPELINE BLOCKED ---

Task:    T-NNN
Phase:   [IMPLEMENT | TEST | REVIEW]
Reason:  [description]
Retry Count: N of 3

Action required: [what the user needs to do]
```

---

## Non-Script Fallback

If the script cannot be run (e.g. macOS / Linux), run:
```
C:\Code\Golem-Agents-Legion/scripts/gal.sh dispatch golem-implementer
```
(and equivalent for tester, reviewer, verifier)

Or invoke each golem directly by asking the user to switch to the appropriate AI model and following the respective agent file:
- `agent/golem-implementer.agent.md`
- `agent/golem-tester.agent.md`
- `agent/golem-reviewer.agent.md`
- `agent/golem-verifier.agent.md`

---

## Stop Conditions Reference

| Condition | Action |
| --- | --- |
| BLOCKING security vuln or Protected Path | STOP immediately — human required |
| `Test Retry Count` reaches 3 | STOP before 4th attempt — human required |
| `Review Retry Count` reaches 3 | STOP before 4th attempt — human required |
| Verifier returns GAPS_FOUND or BLOCKED | STOP — surface gaps, human required |
| Working-hours boundary active before next task | STOP — offer wrap-up once, wait for confirmation |
| `stop-at T-NNN` reached | STOP — prompt user before continuing |
| All tasks + verifier VERIFIED | Natural completion — READY FOR RELEASE |
