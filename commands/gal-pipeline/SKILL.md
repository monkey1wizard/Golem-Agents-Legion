---
name: gal-pipeline
description: "Task-driven autopilot. Iterates through every T-NNN task in the active plan running implement → test → review per task, inserts a conditional `golem-security` audit for security-sensitive implemented changes, keeps a mandatory git commit gate between tasks, and runs a final verifier pass at the end. Stops only on human-required blockers, retry ceiling breach, working-hours boundary, or a user-specified stop boundary."
---

# /gal-pipeline

Run the full implementation pipeline task by task: for each `T-NNN` task in the active plan, run implement → commit → test → review in sequence, insert a conditional `golem-security` audit when the implemented change is security-sensitive, then advance to the next task. Each core phase uses a different AI vendor per `model-roles.local.md`. After all tasks complete, run a final verifier pass.

## Role

Pipeline orchestrator. Your job is to iterate through plan tasks automatically, advancing only when each task's commit + test + review gate, plus any required conditional security audit gate, is fully clean, and stopping only when a genuine human-required condition is encountered.

## When to Use

- After the engineering review lane has produced a `## Tasks` section and a `## Test Plan`
- When you want full task-by-task automation without manual intervention
- When the user says "start implementation", "run the pipeline", "implement and test", or similar

## Syntax

```text
/gal pipeline [#file:<plan.md>] [from T-NNN] [stop-at T-NNN]
```

- **`#file:<plan.md>`**: use the referenced plan file as the pipeline input for this invocation. When present, it overrides `.dev/state.md` active-plan lookup for Step 1 only.
- **No arguments**: start from the first unchecked task, run until all tasks complete
- **`from T-NNN`**: start from the specified task (skip earlier unchecked tasks)
- **`stop-at T-NNN`**: after completing `T-NNN`, stop before starting the next task and prompt the user
- **Resume**: if `Current Task` is set in `## Status`, resume from that task (overridden by explicit `from`)

### xmachine Syntax

Remote pipeline execution is active only when the activation phrase contains both:

- the literal keyword `xmachine`
- a valid node alias from `xmachine.config.json`

Accepted examples:

- `/gal pipeline --xmachine node-name`
- `run the pipeline on xmachine node-name`

Rejected examples:

- `/gal pipeline on node-name`
- `/gal pipeline --xmachine`

## Model Assignment

Each phase uses a different AI vendor, enforced by `model-roles.local.md`:

| Phase | Golem | Role | Why different |
| --- | --- | --- | --- |
| Implement | `golem-implementer` | CODER | Writes the code |
| Test | `golem-tester` | TESTER | Must not read implementation — writes tests from spec only |
| Review | `golem-reviewer` | REVIEWER | Must differ from CODER — fresh eyes on bugs and architecture |
| Verify | `golem-verifier` | VERIFIER | Must differ from CODER — goal-backward plan verification |

`golem-security` is not an always-on fifth pipeline phase. It remains a domain specialist that `/gal pipeline` dispatches only when the implemented change touches auth, data storage or sensitive data handling, user input processing, public API surface, or deployment and environment trust boundaries.

## xmachine Mode

If the dispatcher emits both `EXECUTION: xmachine` and `WORK_NODE: node-name`, keep the pipeline orchestrator on the control node and offload only bounded phase work to the specified node.

Use the dispatcher path resolved by `/gal`'s invoke rules for every script-dispatched golem call in this procedure. If the target project has no repo-local `scripts/` directory, keep the terminal in the target project root and run scripts from the GAL runtime checkout.

Rules:

- Do not send the entire pipeline to the work node.
- Use the GAL runtime checkout's `scripts/Invoke-XmachineTask.ps1 -WorkNode node-name -TaskSpec <phase-task-spec> -Wait` for supported bounded phases when no repo-local wrapper exists.
- Retrieve and inspect `status.json`, `summary.md`, `runtime.log`, and `result.patch` on the control node.
- Apply any returned patch only on the control-node checkout.
- Keep commit gates, plan-state convergence, and protected-path escalation local.

---

## Step 1 — Read Plan and Verify Prerequisites

Select the plan file using this precedence order:

1. If the dispatcher emitted `PLAN: <path>`, use that explicit plan file for this invocation.
2. Otherwise read the active plan file from `.dev/state.md`.

If an explicit `PLAN` path was provided but the file does not exist or is not a markdown plan/prompt file, stop and surface the exact path error.

Verify:

- `## Tasks` exists with at least one `T-NNN` task
- `## Test Plan` exists in the plan file (required for golem-tester)
- No unresolved `BLOCKING` items in `## Review Results` at the root level
- Workflow state is not already `DONE`

If `Current Task` is set in `## Status` and no `from` argument was given, resume from that task.

If prerequisites are not met: tell the user what is missing and stop. If the execution prompt is still stubbed, run `/refining-plan` on the source plan and then rerun `/plan-to-prompt` before attempting the pipeline again.

If xmachine mode is requested, also verify that the selected node is already `readied`. If not, stop and instruct the user to run the GAL runtime checkout's `scripts/Test-Xmachine.ps1 -WorkNode node-name -Wait` first.

---

## Step 2 — Task Loop

Repeat for each unchecked `T-NNN` task (in order, respecting `from` / `stop-at`):

### 2a — Working Hours Check

Before starting a new task, check the current time against the working-hours policy in `conventions/working-hours.md`.

- If the working-hours boundary is active: do not start the next task. Offer `/gal wrap-up` once (do not auto-run it). Wait for explicit user confirmation before proceeding. Stop here.
- If the working-hours boundary is not active: continue.

### 2b — Update Cursor

Update plan `## Status`:

```text
Current Task: T-NNN
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0
Workflow: IMPLEMENT
```

### 2c — Implement (CODER model)

Run:

```powershell
.\scripts\gal.ps1 dispatch golem-implementer
```

Invoke with `TASK_SCOPE: T-NNN`. The implementer must:

1. Record `Task Base Commit` in `## Status` before any changes
2. Implement only the work required by `T-NNN`
3. Record `Task Final Commit` in `## Status` when done
4. Ensure `git status` is clean before reporting complete

**Hard Commit Gate:** If `git status` is not clean or `Task Final Commit` is not recorded, do not proceed. Stop and surface the issue.

**xmachine mode:** if active, offload only the bounded implement slice for `T-NNN` to the selected work node, then retrieve and apply the returned patch on the control node before checking the hard commit gate.

### 2d — Test (TESTER model — different vendor from CODER)

Update plan `## Status`: set `Workflow: TEST`

Run:

```powershell
.\scripts\gal.ps1 dispatch golem-tester
```

Invoke in task-scoped mode for `T-NNN`. The tester writes a `### [T-NNN] YYYY-MM-DD` subsection under `## Test Results`.

Check result:

- **All tests PASS**: update `## Status` `Workflow: REVIEW`, proceed to 2e
- **Any tests FAIL**:
  - Increment `Test Retry Count` in `## Status`
  - If `Test Retry Count` < 3: dispatch implementer to fix failing tests (TASK_SCOPE: T-NNN, fix mode), then re-run tester
  - If `Test Retry Count` = 3: **STOP**. Surface failures. Tell user the retry ceiling (3) has been reached for `T-NNN` and request human intervention

**xmachine mode:** if active, offload only the bounded test task and converge any plan-section or artifact changes on the control node before deciding PASS/FAIL.

### 2e — Review (REVIEWER model — different vendor from CODER and TESTER)

Run:

```powershell
.\scripts\gal.ps1 dispatch golem-reviewer
```

Invoke in task-scoped mode for `T-NNN` with commit range `Task Base Commit..Task Final Commit`. The reviewer writes a `### [T-NNN] YYYY-MM-DD` subsection under `## Review Results`.

Check result:

- **APPROVE (no BLOCKING)**: proceed to 2f
- **REQUEST_CHANGES or BLOCK (BLOCKING findings)**:
  - Increment `Review Retry Count` in `## Status`
  - If `Review Retry Count` < 3: dispatch implementer to fix BLOCKING issues (TASK_SCOPE: T-NNN, fix mode), update `Task Final Commit`, then re-run reviewer
  - If `Review Retry Count` = 3: **STOP**. Surface BLOCKING findings. Tell user the retry ceiling (3) has been reached for `T-NNN` and request human intervention

**Security / Protected Path escalation:** If any BLOCKING finding is a security vulnerability or Protected Path violation, **STOP immediately** regardless of retry count. Do not attempt an automated fix. Surface to human.

**xmachine mode:** if active, offload only the bounded review task, then apply any review-result plan updates on the control node before evaluating APPROVE/BLOCKING.

### 2f — Conditional Security Audit

Decide whether `T-NNN` needs a specialist security pass.

Run `golem-security` only when the implemented change touches one or more of these surfaces:

- authentication
- data storage or sensitive data handling
- user input processing
- public API surface
- deployment or environment trust boundaries

If none apply: skip this step and proceed to 2g.

If any apply, run:

```powershell
.\scripts\gal.ps1 dispatch golem-security
```

Invoke in task-scoped mode for `T-NNN` with commit range `Task Base Commit..Task Final Commit`. The security specialist writes a task-scoped subsection under `## Review Results`.

Check result:

- **Security review clear**: proceed to 2g
- **High or critical findings remain open**: **STOP immediately**. Do not auto-fix inside the pipeline. Surface the findings and request human intervention before task closeout

**xmachine mode:** if active, security audit remains a bounded offload and its returned results must be converged locally before continuing.

### 2g — Mark Task Complete

All gates passed for `T-NNN`:

1. Mark `T-NNN` as complete in `## Tasks` (check the checkbox)
2. Update plan `## Status`:

  ```text
   Last activity: YYYY-MM-DD — T-NNN complete (commit: <Task Final Commit>)
   ```

1. If `stop-at T-NNN` was specified and this task matches: **STOP**. Report task complete and prompt user before starting the next task.
2. Otherwise: advance to the next unchecked task and return to 2a.

---

## Step 3 — Post-Loop Verifier

After all unchecked tasks are complete, dispatch `golem-verifier` for a plan-level goal-backward verification pass.

**IMPORTANT:** Invoke `golem-verifier` for **Steps 1–4 only** (produce a VERIFIED / GAPS_FOUND / BLOCKED verdict). Do **NOT** trigger Step 5 (lifecycle ending: ABSORBED marking + plan file deletion) — that remains a post-release action.

Run:

```powershell
.\scripts\gal.ps1 dispatch golem-verifier
```

Instruct the verifier explicitly: "Run Steps 1–4 only. Do not mark the plan ABSORBED or delete the plan file."

Check result:

- **VERIFIED**: proceed to Step 4 (final gate)
- **GAPS_FOUND**: **STOP**. Surface each gap with its description. Tell user to resolve the gaps before handing off to `golem-releaser`.
- **BLOCKED**: **STOP**. Surface the blocking condition. Tell user to resolve before release work starts.

**xmachine mode:** if active, verifier may run as a bounded offload, but its verdict must still be read and enforced on the control node.

---

## Step 4 — Final Gate

Report the combined verdict:

```text
--- PIPELINE COMPLETE ---

Tasks completed: N of N
  T-001  ✓ implement · test · review
  T-002  ✓ implement · test · review · security (when run)
  ...

Verifier: VERIFIED

Overall: READY FOR RELEASE
```

Tell the user to route the next step to `golem-releaser`.

Also remind the user to rerun `/graphify .` before the next graph-aware planning or review pass so `graphify-out/` reflects the implementation that just completed.

If any task or verifier is blocked, report with detail:

```text
--- PIPELINE BLOCKED ---

Task:    T-NNN
Phase:   [IMPLEMENT | TEST | REVIEW | SECURITY]
Reason:  [description]
Retry Count: N of 3

Action required: [what the user needs to do]
```

---

## Non-Script Fallback

If the script cannot be run (e.g. macOS / Linux), run:

```bash
./scripts/gal.sh dispatch golem-implementer
```

(and equivalent for tester, reviewer, security, verifier)

Or invoke each golem directly by asking the user to switch to the appropriate AI model and following the respective agent file:

- `agent/golem-implementer.agent.md`
- `agent/golem-tester.agent.md`
- `agent/golem-reviewer.agent.md`
- `agent/golem-security.agent.md`
- `agent/golem-verifier.agent.md`

---

## Stop Conditions Reference

| Condition | Action |
| --- | --- |
| BLOCKING security vuln or Protected Path | STOP immediately — human required |
| Conditional security audit leaves high or critical findings open | STOP immediately — human required |
| `Test Retry Count` reaches 3 | STOP before 4th attempt — human required |
| `Review Retry Count` reaches 3 | STOP before 4th attempt — human required |
| Verifier returns GAPS_FOUND or BLOCKED | STOP — surface gaps, human required |
| Working-hours boundary active before next task | STOP — offer wrap-up once, wait for confirmation |
| `stop-at T-NNN` reached | STOP — prompt user before continuing |
| All tasks + verifier VERIFIED | Natural completion — READY FOR RELEASE |

---

## Pipeline Discipline

These rules apply to every phase of the pipeline loop.

### Bounded Output

When a golem phase (implement, test, review, security) returns output, the pipeline must not pipe raw transcripts or full build logs into the orchestration context window. Accept and propagate only:

- A structured phase verdict (PASS / FAIL / APPROVE / BLOCKING / CLEAR)
- Failing items: test names + assertion messages, review BLOCKING findings with file:line references, or build errors with file:line references
- A one-line summary on clean pass

Store raw output as artifacts on disk when needed. Retrieve specific lines or excerpts on demand rather than forwarding entire logs.

### Failure-Focused Evidence

When surfacing a STOP to the user, include only:

- The task ID and phase that failed
- Failing test names and error messages (not passing names)
- First build error with file and line reference (not the full build transcript)
- BLOCKING review findings with file:line citation and severity (not passing dimensions)

### Capability Preflight

Before dispatching any optional lane (xmachine offload, MCP tool, external CLI), resolve readiness via `docs/collaborative-tools/checking-contract.md`.

- If the lane is **`ready`**: proceed.
- If the lane is **`not-ready`** or **`unavailable`**: degrade to the documented fallback (script fallback → non-script fallback → manual instruction) without blocking the pipeline on the missing capability.
- Do not record machine-local lane state in the plan file. Availability is resolved at dispatch time, not at plan-writing time.
