---
name: gal-pipeline
description: "Task-driven autopilot. Iterates through every T-NNN task in the active plan running implement → test → review per task, inserts a conditional `golem-security` audit for security-sensitive implemented changes, keeps a mandatory git commit gate between tasks, and runs a final verifier pass at the end. Stops only on human-required blockers, retry ceiling breach, working-hours boundary, or a user-specified stop boundary."
---

# /gal-pipeline

Run the full implementation pipeline task by task: for each blocking `T-NNN` task in the active plan, run implement → commit → test → review in sequence, insert a conditional `golem-security` audit when the implemented change is security-sensitive, then advance to the next task. Prefer different AI vendors per `model-roles.local.md` when the active runtime can actually enforce that split. After all blocking tasks complete, run a final verifier pass.

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

- **`#file:<plan.md>`**: use the referenced plan file as the pipeline input for this invocation. When present, it overrides `.dev/state.md` active-plan lookup for Step 1 only. If the referenced file is a source plan and the matching `.dev/plans/<slug>.prompt.md` exists, resolve to the execution prompt before continuing.
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

If the dispatcher emits `TASK_REF`, `FROM`, and `STOP_AT`, treat them as authoritative for this invocation. A shorthand such as `/gal xmachine node-name to do TP-007` is expected to arrive here as a bounded single-task control-plane run with `FROM=TP-007` and `STOP_AT=TP-007`.

## Model Assignment

Prefer a different AI vendor for each phase, using `model-roles.local.md` as the desired role mapping and the active runtime config as the enforcement surface:

| Phase | Golem | Role | Why different |
| --- | --- | --- | --- |
| Implement | `golem-implementer` | CODER | Writes the code |
| Test | `golem-tester` | TESTER | Must not read implementation — writes tests from spec only |
| Review | `golem-reviewer` | REVIEWER | Must differ from CODER — fresh eyes on bugs and architecture |
| Verify | `golem-verifier` | VERIFIER | Must differ from CODER — goal-backward plan verification |

`golem-security` is not an always-on fifth pipeline phase. It remains a domain specialist that `/gal pipeline` dispatches only when the implemented change touches auth, data storage or sensitive data handling, user input processing, public API surface, or deployment and environment trust boundaries.

### Runtime Preflight

Before starting the task loop, resolve model separation in this order:

1. Treat runtime-enforced per-agent model routing as authoritative.
2. Treat `model-roles.local.md` as the desired separation policy, not proof that the current runtime can enforce it.
3. If the active runtime cannot prove separate CODER, TESTER, REVIEWER, and VERIFIER routes, degrade explicitly to same-runtime fallback.

OpenCode-specific rule:

- Different models only count when the active OpenCode configuration assigns agent-specific `model` values.
- Inherited subagent execution under the same primary agent model does not satisfy independent verification.
- If no verified per-agent model split exists, OpenCode must follow the documented same-runtime fallback instead of pretending multi-model verification is available.

### Runtime Step-Budget Preflight

Provider turn limits and OpenCode agent `steps` limits are hard runtime boundaries. GAL cannot remove them, so the pipeline must avoid treating a provider cutoff as a workflow decision.

Before starting the task loop:

1. If the active runtime is OpenCode, inspect the nearest repo `opencode.json` when present and note the active agent step budget when it is visible.
2. In OpenCode, enter **single-task tranche mode** by default. Only disable it when the user explicitly asks for a multi-task turn and the visible active-agent `steps` budget is high enough for that larger run.
3. If the active OpenCode agent is `build` and its `steps` value is `20` or lower, warn that even one full task may exceed the runtime budget and rely on the interrupted-phase handoff if the cutoff still happens.
4. In single-task tranche mode, complete at most one blocking task per invocation, including implement, test, review, and any required security pass. After marking that task complete, stop cleanly and tell the user to rerun `/gal pipeline` to continue from the next unchecked task.

This is a normal continuation strategy, not a BLOCKED state. It prevents low-step agents from finishing multiple tasks and then being cut off mid-implementation on the next one.

If a runtime cutoff still occurs mid-phase and the next invocation sees `Workflow: IMPLEMENT`, `TEST`, `REVIEW`, `SECURITY`, or `VERIFY` with incomplete phase write-back, treat it as an interrupted phase and resume that phase before considering any new task.

## xmachine Mode

If the dispatcher emits both `EXECUTION: xmachine` and `WORK_NODE: node-name`, keep the pipeline orchestrator on the control node and offload only bounded phase work to the specified node.

Use the dispatcher path resolved by `/gal`'s invoke rules for every script-dispatched golem call in this procedure. If the target project has no repo-local `scripts/` directory, keep the terminal in the target project root and run scripts from the GAL runtime checkout.

Rules:

- Do not send the entire pipeline to the work node.
- If the dispatcher emits `OFFLOAD: direct-task`, use the GAL runtime checkout's `scripts/Invoke-XmachineTask.ps1 -WorkNode node-name -TaskSpec <phase-task-spec> -Wait` for the bounded phase; do not use `Invoke-XmachinePipeline.ps1` / `Invoke-XmachinePipeline.sh`.
- If the dispatcher emits `XMACHINE_MODE: execute`, do not pass `-WorkRepoPath` unless the target repo has an intentional persistent checkout on the work node.
- In `XMACHINE_MODE: execute`, make the phase task spec self-contained: include the exact file contents, minimal reproduction commands, or explicit temporary-materialization instructions needed on the work node. Do not tell the work node to inspect local control-node paths or stale remote repo paths.
- Use the GAL runtime checkout's `scripts/Invoke-XmachineTask.ps1 -WorkNode node-name -TaskSpec <phase-task-spec> -Wait` for supported bounded phases when no repo-local wrapper exists.
- Retrieve and inspect `status.json`, `summary.md`, `runtime.log`, and `result.patch` on the control node.
- Apply any returned patch only on the control-node checkout.
- Keep commit gates, plan-state convergence, and protected-path escalation local.

---

## Step 1 — Read Plan and Verify Prerequisites

Select the plan file using this precedence order:

1. If the dispatcher emitted `PLAN: <path>`, resolve that explicit path first. If it points to a source plan and the matching `.dev/plans/<slug>.prompt.md` exists, use the execution prompt for this invocation.
2. Otherwise read the active plan file from `.dev/state.md`.

If the dispatcher emitted `TASK_REF`, validate that the referenced task exists in the selected plan before entering the task loop. If `FROM` and `STOP_AT` are both present, use them as the explicit execution bounds even when the user did not type `from` / `stop-at` directly in chat.

If an explicit `PLAN` path was provided but the file does not exist or is not a markdown plan/prompt file, stop and surface the exact path error.

The filesystem is authoritative for explicit plan resolution:

- If the explicit path already points to `.dev/plans/<slug>.prompt.md`, use it directly.
- If the explicit path points to `docs/plans/<slug>.md` and `.dev/plans/<slug>.prompt.md` exists, switch to the prompt and use that as the pipeline file.
- If the explicit path points to a source plan and no prompt exists yet, use the source plan only to detect missing prerequisites, then stop with the required `/refining-plan` and `/plan-to-prompt` guidance instead of trying to execute against the source plan.

Verify:

- `## Tasks` exists with at least one blocking `T-NNN` task
- `## Test Plan` exists in the plan file (required for golem-tester)
- No unresolved `BLOCKING` items in `## Review Results` at the root level
- Workflow state is not already `DONE`
- Any deferred or non-blocking follow-up lives outside `## Tasks` and is not used as a pipeline loop gate

If `Current Task` is set in `## Status` and no `from` argument was given, resume from that task. If `### Handoff Notes` contains an `OPEN` `Interrupted Phase` block, or if `Workflow` names a phase whose convergence gate is incomplete, resume that phase first.

Never run Step 2 task execution directly against a source plan when the matching execution prompt exists. `## Status`, retry counters, commit checkpoints, handoff notes, and task completion write-back belong in `.dev/plans/<slug>.prompt.md`.

If prerequisites are not met: tell the user what is missing and stop. If the execution prompt is still stubbed, run `/refining-plan` on the source plan and then rerun `/plan-to-prompt` before attempting the pipeline again.

If xmachine mode is requested, also verify that the selected node is already `readied`. If not, stop and instruct the user to run the GAL runtime checkout's `scripts/Test-Xmachine.ps1 -WorkNode node-name -Wait` first.

### Retry And Blocker Handoff Contract

The active execution prompt's `## Status > ### Handoff Notes` is the durable human-takeover surface for pipeline failures. Do not leave takeover detail only in chat output.

When a task hits repeated failure or an immediate human-required stop, append or refresh a single task-scoped block in `### Handoff Notes` using this format:

```markdown
#### Retry Handoff — T-NNN / [TEST | REVIEW | SECURITY | XMACHINE]

- Status: OPEN | RESOLVED
- Problem: <latest blocking problem statement>
- Evidence:
  - Test Results: <latest task-scoped subsection or `not-applicable`>
  - Review Results: <latest task-scoped subsection or `not-applicable`>
  - Security Review: <latest task-scoped subsection or `not-applicable`>
  - xmachine Artifacts: <`gal-results/<task-id>/status.json`, `summary.md`, `runtime.log`, `result.patch` or `not-applicable`>
- Attempts:
  1. <YYYY-MM-DD> — <attempt summary>
     - Result: <what changed or why it still failed>
     - Validation: <command, reviewer verdict, or `not-run`>
     - Commit: <hash or `none`>
  2. ...
- Next human step: <exact next inspection or repair step>
```

Rules:

- Keep exactly one `OPEN` handoff block per `Current Task` and active phase. Update the existing block instead of appending duplicates.
- Record every retry-triggered fix attempt in order. By the third failed `TEST` or `REVIEW` round, the handoff must tell the human what was tried on attempts 1-3 without reconstructing history from chat.
- For `SECURITY` and `XMACHINE`, write the same handoff format on the first stop even when no retry loop is involved.
- When a later rerun clears the issue, keep the block for history but change `Status` to `RESOLVED` and replace `Next human step` with the confirmation that cleared it.

### Interruption Handoff Contract

Use interruption handoff for non-decision runtime cutoffs such as OpenCode `steps` exhaustion, provider max-turn limits, context exhaustion, or terminal/tool availability ending a phase before its convergence gate is reached.

If the current invocation receives a runtime message equivalent to "maximum steps reached" or resumes and finds an incomplete current phase, write or refresh this block before doing any unrelated work:

```markdown
#### Interrupted Phase — T-NNN / [IMPLEMENT | TEST | REVIEW | SECURITY | VERIFY]

- Status: OPEN | RESOLVED
- Cause: runtime-step-limit | context-limit | tool-unavailable | unknown
- Workflow at interruption: <Workflow value>
- Durable state present:
  - Task Base Commit: <hash or `missing`>
  - Task Final Commit: <hash or `missing`>
  - Worktree: clean | dirty | unknown
  - Test Results: <task-scoped subsection present? yes/no/not-applicable>
  - Review Results: <task-scoped subsection plus verdict present? yes/no/not-applicable>
- Resume action: <exact next command or phase action>
```

Rules:

- Keep exactly one `OPEN` interrupted-phase block per task and phase. Update it instead of appending duplicates.
- An interrupted phase is not the same as a failed phase. Do not increment `Test Retry Count` or `Review Retry Count` unless a real failing test or review finding exists.
- On resume, clear the interrupted-phase block only after the phase's normal convergence gate succeeds.
- If the interrupted phase is `IMPLEMENT` and the worktree is dirty, continue from the existing changes, run the scoped verification, commit, and record `Task Final Commit`. Do not start a new task.

---

## Step 2 — Task Loop

Repeat for each unchecked blocking `T-NNN` task in `## Tasks` (in order, respecting `from` / `stop-at` and any single-task tranche mode):

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
Next step: implement T-NNN
```

If `### Handoff Notes` contains an `OPEN` retry handoff for a previous task, mark it `RESOLVED` before starting the new task. Do not carry stale blocker state across tasks.

### 2c — Implement (CODER model)

Run:

```powershell
.\scripts\gal.ps1 dispatch golem-implementer --pipeline-phase implement --task-scope T-NNN
```

The dispatcher must emit `MODE: bound`, `DISPATCH_KIND: pipeline-phase`, `PIPELINE_PHASE: implement`, and `TASK_SCOPE: T-NNN`. The implementer must:

1. Record `Task Base Commit` in `## Status` before any changes
2. Set `Current Task: T-NNN` in `## Status` before reporting any implementation progress
3. Implement only the work required by `T-NNN`
4. Record `Task Final Commit` in `## Status` when done
5. Ensure `git status` is clean before reporting complete
6. In pipeline fix mode, update the active `Retry Handoff` block in `### Handoff Notes` with the attempted remediation, validation result, and commit hash (if any)

**Hard Commit Gate:** If `Current Task` is missing or points at a different task, `git status` is not clean, or `Task Final Commit` is not recorded, do not proceed. Stop and surface the missing write-back instead of inferring completion from chat alone.

**xmachine mode:** if active, offload only the bounded implement slice for `T-NNN` to the selected work node, then retrieve and apply the returned patch on the control node before checking the hard commit gate. If the retrieved `status.json` is not `success`, **STOP immediately**. Write a `Retry Handoff — T-NNN / XMACHINE` block with the xmachine task id, exit code, `errorMessage`, local artifact paths under `gal-results/<task-id>/`, whether `result.patch` was left unapplied, and the exact next human inspection step.

### 2d — Test (TESTER model — different vendor from CODER)

Update plan `## Status`: set `Workflow: TEST`

Run:

```powershell
.\scripts\gal.ps1 dispatch golem-tester --pipeline-phase test --task-scope T-NNN
```

The dispatcher must emit `MODE: bound`, `DISPATCH_KIND: pipeline-phase`, `PIPELINE_PHASE: test`, and `TASK_SCOPE: T-NNN`. The tester writes a `### [T-NNN] YYYY-MM-DD` subsection under `## Test Results`.

Check result:

- **No task-scoped subsection was written**: **STOP immediately**. Write `Retry Handoff — T-NNN / TEST` with the missing write-back as the problem. Do not infer PASS or FAIL from chat alone.
- **`Workflow: TEST` is set but the latest task-scoped subsection is still missing or placeholder-only**: **STOP immediately**. Treat this as incomplete durable state, not as a passing or failing run.
- **All tests PASS**: update `## Status` `Workflow: REVIEW`, proceed to 2e
- **Any tests FAIL**:
  - Increment `Test Retry Count` in `## Status`
  - Refresh the active `Retry Handoff — T-NNN / TEST` block with the latest failing test names, the current `## Test Results` subsection, and the next fix target
  - If `Test Retry Count` < 3: dispatch implementer to fix failing tests with `--pipeline-phase implement --task-scope T-NNN --fix-mode`, then re-run tester
  - If `Test Retry Count` = 3: **STOP**. Surface failures. Tell user the retry ceiling (3) has been reached for `T-NNN`, include attempts 1-3 from the handoff block, and request human intervention

If the tests pass after one or more failed rounds, mark `Retry Handoff — T-NNN / TEST` as `RESOLVED` and note the validation run that cleared it.

**xmachine mode:** if active, offload only the bounded test task and converge any plan-section or artifact changes on the control node before deciding PASS/FAIL. If the retrieved `status.json` is not `success`, **STOP immediately** and write `Retry Handoff — T-NNN / XMACHINE` with the failed phase, task id, exit code, error message, and local artifact paths.

### 2e — Review (REVIEWER model — different vendor from CODER and TESTER)

Run:

```powershell
.\scripts\gal.ps1 dispatch golem-reviewer --pipeline-phase review --task-scope T-NNN
```

The dispatcher must emit `MODE: bound`, `DISPATCH_KIND: pipeline-phase`, `PIPELINE_PHASE: review`, and `TASK_SCOPE: T-NNN`. Invoke in task-scoped mode for `T-NNN` with commit range `Task Base Commit..Task Final Commit`. The reviewer writes a `### [T-NNN] YYYY-MM-DD` subsection under `## Review Results`.

Check result:

- **No task-scoped subsection or verdict was written**: **STOP immediately**. Write `Retry Handoff — T-NNN / REVIEW` with the missing write-back as the problem. Do not infer approval or block from chat alone.
- **`Workflow: REVIEW` is set but the latest task-scoped subsection still has no verdict**: **STOP immediately**. Treat this as incomplete durable state, not as approval.
- **APPROVE (no BLOCKING)**: proceed to 2f
- **REQUEST_CHANGES or BLOCK (BLOCKING findings)**:
  - Increment `Review Retry Count` in `## Status`
  - Refresh the active `Retry Handoff — T-NNN / REVIEW` block with the latest open BLOCKING findings, current review subsection, and the next fix target
  - If `Review Retry Count` < 3: dispatch implementer to fix BLOCKING issues with `--pipeline-phase implement --task-scope T-NNN --fix-mode`, update `Task Final Commit`, then re-run reviewer
  - If `Review Retry Count` = 3: **STOP**. Surface BLOCKING findings. Tell user the retry ceiling (3) has been reached for `T-NNN`, include attempts 1-3 from the handoff block, and request human intervention

**Security / Protected Path escalation:** If any BLOCKING finding is a security vulnerability or Protected Path violation, **STOP immediately** regardless of retry count. Do not attempt an automated fix. Surface to human.

If the review passes after one or more failed rounds, mark `Retry Handoff — T-NNN / REVIEW` as `RESOLVED` and note the reviewer pass that cleared it.

**xmachine mode:** if active, offload only the bounded review task, then apply any review-result plan updates on the control node before evaluating APPROVE/BLOCKING. If the retrieved `status.json` is not `success`, **STOP immediately** and write `Retry Handoff — T-NNN / XMACHINE` with the failed phase, task id, exit code, error message, and local artifact paths.

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
- **High or critical findings remain open**: **STOP immediately**. Do not auto-fix inside the pipeline. Write `Retry Handoff — T-NNN / SECURITY` with the open findings, affected files, current remediation status, and the next human step before task closeout

**xmachine mode:** if active, security audit remains a bounded offload and its returned results must be converged locally before continuing. If the retrieved `status.json` is not `success`, **STOP immediately** and write `Retry Handoff — T-NNN / XMACHINE` with the failed phase, task id, exit code, error message, and local artifact paths.

### 2g — Mark Task Complete

All gates passed for `T-NNN`:

1. Mark `T-NNN` as complete in `## Tasks` (check the checkbox)
2. Update plan `## Status`:

  ```text
   Last activity: YYYY-MM-DD — T-NNN complete (commit: <Task Final Commit>)
   ```

1. If `stop-at T-NNN` was specified and this task matches: **STOP**. Report task complete and prompt user before starting the next task.
2. If single-task tranche mode is active: **STOP CLEANLY**. Report task complete, state that this is a runtime step-budget tranche, and tell the user to rerun `/gal pipeline` to continue.
3. Otherwise: advance to the next unchecked task and return to 2a.

---

## Step 3 — Post-Loop Verifier

After all unchecked tasks are complete, dispatch `golem-verifier` for a plan-level goal-backward verification pass.

**IMPORTANT:** Invoke `golem-verifier` for **Steps 1–4 only** (produce a VERIFIED / GAPS_FOUND / BLOCKED verdict). Do **NOT** trigger Step 5 (lifecycle ending: ABSORBED marking + plan file deletion) — that remains a post-release action.

Run:

```powershell
.\scripts\gal.ps1 dispatch golem-verifier --pipeline-phase verify
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
Phase:   [IMPLEMENT | TEST | REVIEW | SECURITY | XMACHINE]
Reason:  [description]
Retry Count: N of 3 | not-applicable

Attempts tried:
1. [attempt summary]
2. [attempt summary]
3. [attempt summary]

Evidence to inspect:
- Handoff Notes: `## Status > ### Handoff Notes`
- Test Results: [latest task-scoped subsection or `not-applicable`]
- Review Results: [latest task-scoped subsection or `not-applicable`]
- Security Review: [latest task-scoped subsection or `not-applicable`]
- xmachine Artifacts: [`gal-results/<task-id>/status.json`, `summary.md`, `runtime.log`, `result.patch` or `not-applicable`]

Action required: [what the user needs to do]
```

The blocked output must mirror the active `Retry Handoff` block closely enough that a human can answer three questions immediately: what failed, what was tried already, and what artifact or file to inspect next.

---

## Non-Script Fallback

If the script cannot be run (e.g. macOS / Linux), run:

```bash
./scripts/gal.sh dispatch golem-implementer --pipeline-phase implement --task-scope T-NNN
```

(and equivalent phase-marked invocations for tester, reviewer, and verifier; keep `golem-security` as a direct specialist dispatch)

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
| Any xmachine phase returns non-success `status.json` | STOP immediately — write `Retry Handoff — T-NNN / XMACHINE`, do not auto-apply a failed patch |
| `Test Retry Count` reaches 3 | STOP before 4th attempt — human required |
| `Review Retry Count` reaches 3 | STOP before 4th attempt — human required |
| Verifier returns GAPS_FOUND or BLOCKED | STOP — surface gaps, human required |
| Working-hours boundary active before next task | STOP — offer wrap-up once, wait for confirmation |
| `stop-at T-NNN` reached | STOP — prompt user before continuing |
| Single-task tranche complete | STOP cleanly — rerun `/gal pipeline` to continue; not a blocker |
| Runtime step or turn limit interrupts a phase | On next invocation, write or refresh `Interrupted Phase — T-NNN / PHASE` and resume the incomplete phase before new work |
| All tasks + verifier VERIFIED | Natural completion — READY FOR RELEASE |
