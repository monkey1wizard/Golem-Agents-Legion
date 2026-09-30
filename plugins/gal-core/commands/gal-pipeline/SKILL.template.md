---
name: gal-pipeline
description: "GAL pipeline ($gal-pipeline / /gal pipeline / 跑 pipeline / 實作 / implement / run the pipeline). Entry Latch: run gal pipeline-preflight receipt BEFORE any implementation edit. Iterates T-NN tasks: task check -> implement -> commit -> test -> audit -> converge."
---

# /gal-pipeline

Run the blocking `T-NN` tasks in order: task check, implementation, commit, test, audit, and convergence. After all tasks pass, the ORCHESTRATOR runs the in-process goal-backward verification. Use the active execution prompt, not provider-local memory.

## Role

Pipeline orchestrator. Advance only on clean task gates and stop only on a checker-authorized terminal decision or an unrecoverable runtime cutoff.

## Entry Latch (binding — no edit before this passes)

Before any implementation edit, run `gal pipeline-preflight <execution-prompt-path>` and read `.dev/pipeline/<plan-scope-key>/preflight.receipt.md`. Only `overall: pass` permits work; `fail` or `not-run` stops immediately. Resolve an explicit `#file:<prompt>` when more than one plan is active. The receipt is the sole pass-basis.

If the plan touches `crates/`, follow `conventions/self-bootstrap.md` before the first gate command. Re-resolve and hash the executable before trusting each gate receipt; a mismatch stops the run.

## Terminal-Reverify Entry Branch

When finalize routing reports stale goal binding with otherwise clean passing state:

1. Run `gal.exe pipeline-preflight --terminal-reverify <execution-prompt-path>` and require the fresh receipt to pass `Workflow: DONE`, checked tasks, cleared `Current Task`, no open handoff, and a clean worktree.
2. Run only the ORCHESTRATOR-owned in-process goal-backward verification, then a fresh `pipeline-handback-check`.
3. Do not dispatch phases, execute executor work, commit, mutate the prompt, repair evidence, or weaken any gate.

## Pipeline Handback Authority

`gal.exe pipeline-handback-check <execution-prompt-path> [--stop-at T-NN]` is the sole authority for continuation and voluntary final response. Run it after every convergence gate and before every final response. Read these fields only:

The authoritative tuple is exactly `(decision, reason, voluntary_response_authorized, continue_action)`.
`continue_action` is executable only when `decision: continue`; every terminal decision uses the literal `none` under `legacy_interactive`. Under guarded continuation, the receipt also carries `typed_action`, `coordinator_revision`, and `coordinator_commit`; the checker serializes `typed_action` into `continue_action` for compatibility, and the bound `typed_action` is the authority for the next guarded transition. A guarded terminal decision still has no executable action.

```text
decision: continue | goal-verified | human-required | retry-ceiling | stop-at
reason: none | security-protected-path | goal-gaps-blocked | head-drift | boundary-scope-decision | convergence-human-repair
voluntary_response_authorized: true | false
continue_action: executable only under decision: continue; literal none for terminal decisions
```

| decision | authorized | action |
| --- | --- | --- |
| `continue` | false | Execute the one closed `continue_action`. |
| `goal-verified` | true | Stop and return the receipt-backed final response. Do not run `/gal finalize`. |
| `human-required` | true | Stop and return the receipt-backed handback. |
| `retry-ceiling` | true | Stop and return the receipt-backed handback. |
| `stop-at` | true | Stop and return the receipt-backed decision. |

The closed legacy continue actions are `/gal pipeline <prompt> from <task> [stop-at <target>]`, `run-goal-backward-verification`, `repair-stop-at-target <target>`, and `repair-stop-at-convergence <target>`. A pending valid future `stop-at` is a pipeline action with the target preserved. Repair actions require repairing the named state before rerunning the checker. Guarded `typed_action` values follow the typed coordinator action contract below.

Every `human-required` result requires one valid handback block. Keep the handback procedure in `conventions/handoff-notes.md`. For `goal-gaps-blocked`, write and validate that handback before hashing the prompt or writing the goal-verification record.

Runtime cutoff is recovery-only: write or refresh `Interrupted Phase`, leave a rerunnable resume marker, and never authorize final output or schedule a wake-up. The binary owns dispatch markers, attempt logs, terminal states, and other internals; see `docs/workflows.md` and `docs/architecture.md`.

## Codex / PowerShell Invocation Safety

- Quote `'#file:<execution-prompt-path>'` in PowerShell because bare `#` starts a comment.
- With multiple active plans, require the explicit prompt path; never select the first active-plan row.
- If a receipt or executor-log write under `.dev/pipeline/` is denied by the sandbox, stop and rerun the exact command after approval. Do not role-play the phase.
- Fix mode is `gal.exe pipeline <prompt> --phase implement --task T-NN --fix`; unchanged affected files are not a successful retry.
- A Codex functions cell or shell session is a host session, not a dispatch, and follows the host-session rule under `## Headless Executor Dispatch`. The orchestrator never self-elevates or runs an unsandboxed re-check of an executor lookup on its own. A bounded comparison under native approval follows `docs/setup.md` and is granted only by the owner. This bullet does not change the receipt-write rerun bullet above it.

## When to Use

- Use after `## Tasks` and `## Test Plan` exist.
- Use for task-by-task implementation, testing, audit, convergence, and goal verification.

## Syntax

```text
/gal pipeline [#file:<plan.md> | @<plan.md>] [from T-NN] [stop-at T-NN]
```

Resolve an explicit source plan to its paired execution prompt. Without an explicit path, use the active plan in `.dev/state.md`. `from` skips earlier unchecked tasks; `stop-at` stops after the named task.

## Model Assignment

Implement maps to CODER, test to TESTER, and audit to AUDITOR. Verify is ORCHESTRATOR-owned and never dispatched. Supported executors are `claude`, `codex`, `opencode`, `copilot`, and `agy`.

## Continuation Contract

The default `legacy_interactive` profile preserves the v1 invocation and its defaults. The versioned `codex_stop_v1` profile selects guarded continuation; its profile value identifies the contract version and does not select an executor. Do not infer a profile from the executor name.

Dispatch evidence uses `v1` for the existing invocation and marker format. Keep its arguments, defaults, marker grammar, and output order unchanged. `v2` adds bindings for plan scope, prompt digest, task and phase, attempt and session identity, tested commit, working-tree diff, and receipt snapshot. A gate that requires v2 accepts only evidence with the required bindings matching the current task and attempt; legacy v1 evidence remains valid under the legacy profile.

The coordinator's `next_action` is typed by `kind` and `binding`. Actions are `start_task` and `resume_task` with `task_id` and `phase`; `await_orchestrator` with `checkpoint_id`; `recover_attempt` with `attempt_id`; `retry_task` with `task_id`, `phase`, and `attempt`; `complete`; or `blocked` with `reason`. Execute only the single action returned by the current gate. A handback result binds `typed_action` to `coordinator_revision` and `coordinator_commit`; stale or mismatched bindings do not authorize continuation.

The guarded driver stops at semantic checkpoints for ORCHESTRATOR judgment. A task-quality checkpoint must receive a valid, current receipt before work begins. Boundary and convergence checks must pass before verified progress is projected. After the final task converges, the `goal_backward` checkpoint waits for the ORCHESTRATOR's in-process verification. These gates do not delegate verification or transfer loop ownership: the host remains responsible for each next action and each phase dispatch.

## Runtime Preflight

The preflight route line identifies the resolved executor and model. Keep implement, test, audit, and verify as separate bounded phases. Same-runtime bundling is allowed only when explicitly requested and must record `Verification Independence: DEGRADED_BUNDLED`. Never waive retry ceilings, protected-path escalation, audit stops, interrupted-phase handoff, or final verification.

## Loop-Log

For an in-conversation durable failure or degrade, append one failure-only event with `gal pipeline-log append`; this writes `.dev/pipeline/<yyyymmdd>/loop-log.ndjson` and is forensic convenience, not a workflow gate.

## Headless Executor Dispatch

When routing exists, `gal dispatch-script` emits `OFFLOAD` and `gal pipeline` runs the executor in the binary. Without routing, it emits text dispatch and the orchestrator role-plays the phase. OFFLOAD is keyed by `(task, phase, routing)`. Before dispatch, confirm the prompt matches the active plan in `.dev/state.md`.

The binary owns adapter flags, permission bypasses, `effort`, receipt leases, remote fetches, attempt logs, terminal states, contract delivery, and marker grammar. The write root is `.dev/pipeline/`; plan-scoped logs use `.dev/pipeline/<plan-slug>/<task>/`, and raw dispatch uses `.dev/pipeline/<yyyymmdd>/test-direct/`. Read `docs/workflows.md` for the operational contract and `docs/architecture.md` for binary internals.

**Receipt freshness is dispatcher-owned.** Before a local executor starts, the dispatcher acquires a lease under `<workdir>/.dev/pipeline/.locks/` and holds it through receipt verification. Only a `*.receipt.md` file under `.dev/pipeline/` and outside `.dev/pipeline/.locks/` is dispatcher-managed; every other pre-existing path fails closed and is never deleted.

**Executor output belongs to the task.** Changes an executor leaves in the working tree, tracked and untracked alike, belong to the task. The orchestrator never deletes them, never reverts them with `git checkout`, `git restore`, `git reset`, or `git clean`, and never stashes them. Only the owner may authorize discarding them. When the latest attempt log does not read `completed`, including a `started` marker left when the dispatch process was killed, do not advance the phase, keep the output in place, and re-dispatch the phase once, only after the original dispatch process has exited. If that re-dispatch also ends without `completed`, write a `Human Handback — convergence-human-repair` block per the Human Handback Contract in `plugins/gal-core/conventions/handoff-notes.md` and follow `pipeline-handback-check`. This handback applies when an implement re-dispatch returns `no-writeback` because the kept output already satisfies the task, and when a re-dispatch fails closed because the binary kept a lease under `.dev/pipeline/.locks/`, either the prompt-writer lease or the receipt lease, after a killed or `disconnected-partial` dispatch. A kept prompt-writer lease blocks every dispatch on that prompt. The orchestrator never discards output to make a later dispatch write again, and it never deletes or edits a file under `.dev/pipeline/.locks/` because only the owner clears a stale lease. This duty also holds during Runtime Cutoff Recovery.

**No caller-side dispatch time limit.** The executor's configured `timeoutSecs`, enforced by the binary, is the only time limit on a dispatch. The orchestrator never wraps a dispatch in its own time limit, such as a shell timeout or a tool-call timeout shorter than `timeoutSecs`. When a dispatch may outlast the caller's own command timeout, the orchestrator runs it in the background, waits for the process to exit, and only then applies the Three-Condition PASS Threshold. When a host tool yields control before the dispatch exits, the orchestrator waits on the original session handle and never starts a second dispatch for the same phase. A receipt read before the process exits is not evidence.

**A host session is not a dispatch.** A host session is the orchestrator's own tool call or shell. A dispatch is one binary attempt with its own attempt log. When the orchestrator cannot confirm that the dispatch process exited because the host session was lost or its state is unknown, the orchestrator writes `Interrupted Phase` per Runtime Cutoff Recovery, keeps the output in place, and does not re-dispatch because a second writer could collide with a dispatch that is still running. When a later invocation resumes that phase, the kept-lease rule in the custody paragraph above applies.

**Report the recorded stage, not a cause.** When a dispatch ends before `completed`, report the stage the binary recorded, as observed: availability (`Missing`, `Denied`, `Unknown`), launch, provider response, terminal result, receipt, or gate evidence. These are the stages defined by the binary's availability probe and `docs/workflows.md`. A `Denied` or `Unknown` lookup is not evidence that an executor is missing. The orchestrator never infers a cause or an install state from one, and never elevates permissions on its own to re-check one.
Phase-to-role map: implement -> CODER, test -> TESTER, audit -> AUDITOR. Verify is ORCHESTRATOR-owned. Record the `Dispatch:` line before moving to the next phase or task.

### REPORT_LINE Announcement (exact-once, verbatim)

Announce the rendered `REPORT_LINE` exactly once at every implement, fix, test, and audit dispatch. Do not paraphrase or add dispatch narration.

### Three-Condition PASS Threshold (Honest Test Gate)

A dispatch is PASS only when the current-run receipt is a non-empty regular file, the plan-scoped attempt log under `.dev/pipeline/<plan-slug>/<task>/` (or the raw-dispatch log under `.dev/pipeline/<yyyymmdd>/test-direct/`) records terminal state `completed`, and the provider-native session record is present. For v2 evidence, require all applicable bindings to match the current task and attempt. Anything less is not PASS.

### Commit Boundary

The secondary CLI must not run `git commit` or `git push`. The orchestrator owns the task commit after the boundary gate and convergence requirements pass.

### Security Warning

Headless permission-bypass flags grant the secondary CLI full filesystem and terminal access. Enable executor routing only in a trusted environment and surface this warning before first use.

### Runtime Cutoff Recovery

`RECOVER —` Write or refresh `Interrupted Phase`, leave a recovery-only rerun marker, and resume the interrupted phase on the next invocation. Provider turn limits and OpenCode `steps` limits are possible runtime cutoffs, not workflow decisions. Do not self-wake or authorize final output.

## Step 1 — Read Plan and Verify Prerequisites

Resolve the prompt, validate any dispatcher `TASK_REF`, `FROM`, and `STOP_AT`, and require `## Tasks`, `## Test Plan`, no root-level `BLOCKING`, and a workflow that is not `DONE`. Keep status and mutable write-back in the execution prompt. Do not execute a source plan when its prompt exists.

Read the preflight receipt and follow `conventions/self-bootstrap.md` for the executable recheck. If preflight fails, stop and surface the exact receipt evidence.

---

## Step 2 — Task Loop

Repeat each unchecked blocking `T-NN` in order. After convergence, run the handback checker and execute its authorized action in the same invocation. Every dispatch spec uses the task and that phase's own agent contract. Follow `conventions/self-bootstrap.md` for the receipt recheck.

### 2a — Working Hours Check

Pipeline execution is **exempt from working-hours by default**; proceed without a working-hours handback.

### 2b — Task Quality Check (ORCHESTRATOR)

Read `conventions/task-quality.md` and require every applicable question to be answered. Missing answers stop the task without write-back or attempt log.

### 2c — Update Cursor

Set `Current Task: T-NN`, commit markers, retry counts, and `Workflow: IMPLEMENT` in the prompt. Resolve stale retry handoff state before starting a new task.

### 2d — Implement, Reconcile, And Commit (CODER / ORCHESTRATOR)

Capture HEAD before dispatch; the executor returns an uncommitted task-scoped diff and must not change HEAD. Require `MODE: bound`, `DISPATCH_KIND: pipeline-phase`, `PIPELINE_PHASE: implement`, and `TASK_SCOPE: T-NN`. The orchestrator runs `gal boundary-check <prompt> --task T-NN`, creates the task commit, records `Task Final Commit`, confirms a clean worktree, and runs the committed-range boundary check. A failed dispatch write-back or boundary receipt stops the task.

**Boundary Widening Protocol (implement only).** When the sole out-of-allowlist change is a visibility-only edit reusing already-landed prior-task logic (confirmed from the `Task Base Commit..HEAD` diff): widen the task's allowlist in the execution prompt with a one-line justification, commit that allowlist edit alone as a doc-only commit, record a `## Status > ### Deviations` row naming the reused symbol and prior task, then re-run `gal boundary-check` for both the task and committed-range checks before advancing. Any other boundary failure is a hard violation — revert or fix scope, never widen.

### 2e — Test (TESTER model — independent test author)

Require `Task Final Commit == HEAD` before dispatch. Skip only when every covering Test Plan row is `grep`, `manual`, or `documentation`; otherwise dispatch the TESTER. The pipeline-bound tester receipt must begin on its first line with `### [T-NN] YYYY-MM-DD`, contain the complete task-scoped test subsection, and contain no metadata or preamble before that heading. The control node validates it and writes it under `## Test Results`; generated test and audit dispatch instructions name the receipt path and require this same heading-first form. Reconcile every covering `TP-NN` outcome under `## Test Results`. A failure increments the retry count and dispatches implement fix mode below the ceiling.

### 2f — Audit (AUDITOR model — independent audit)

Require `Task Final Commit == HEAD`, unchanged HEAD around dispatch, and a task-scoped receipt subsection placed under `## Review Results`. Reconcile the verdict and evidence. APPROVE with no BLOCKING advances; a blocking result retries implement fix mode below the ceiling. Security or Protected Path findings, high or critical findings, and retry ceiling stop immediately.

### 2g — Mark Task Complete And Converge State

Update the source plan, execution prompt, and `.dev/state.md` only after implement, test, and audit pass. Mark `T-NN` checked, record the final commit, clear the cursor, re-read all three surfaces, and require `gal boundary-check <prompt> --task T-NN --boundary-kind state-recording` followed by `gal pipeline-converge-check <prompt> --task T-NN`. Both binary gates must pass. Then run `pipeline-handback-check` and execute its one literal action.

## Step 3 — Orchestrator Goal-Backward Verification

After all tasks converge, consume `run-goal-backward-verification` in-process. Run terminal reverify, hash its receipt into the goal record, verify Truths, Files/Components, and Wiring, run the required tests/builds, and write a bound `VERIFIED` goal record. The record requires continuous non-empty `must_have_1` through `must_have_N` fields. `Workflow` is advisory prompt metadata. The checker validates freshness and state consistency only; it never performs semantic verification.

For `GAPS_FOUND`, write exactly one `Human Handback — goal-gaps-blocked` block before hashing the prompt or writing the goal record. For `BLOCKED`, write the typed handback. Run the checker and follow its receipt.

## Step 4 — Final Gate

On `goal-verified`, return only the receipt-backed terminal response and tell the owner the plan is ready for `/gal finalize`. Do not run `/gal finalize` from this skill.

## Non-Script Fallback

If the script cannot run, use the equivalent `gal dispatch-script` phase invocation with the explicit `#file:` prompt path. With routing, follow the emitted `OFFLOAD`; without routing, role-play the named phase. Verify remains in-process.

## Stop Conditions Reference

Autonomy waives no human-required stop. Runtime cutoff and ordinary progress are recovery/continuation states, not handback authority.

| Condition | Action |
| --- | --- |
| BLOCKING security vuln or Protected Path | STOP immediately — human required |
| Conditional security audit leaves high or critical findings open | STOP immediately — human required |
| `Test Retry Count` reaches 3 | STOP before 4th attempt — human required |
| `Review Retry Count` reaches 3 | STOP before 4th attempt — human required |
| Goal-backward verify returns GAPS_FOUND or BLOCKED | STOP — surface gaps, human required |
| Three-surface convergence gate fails (2g) | STOP immediately — repair convergence before advancing |
| HEAD-drift detected after any dispatch returns | STOP immediately — record Deviation; no automatic revert |
| Post-phase boundary compare (2d/2g) reads `fail` or `not-run` | STOP immediately — follow the phase protocol |
| Dispatch ends without `completed` and leaves working-tree changes | STOP the phase — keep the output in place and re-dispatch once. If it again ends without `completed`, hand back via `Human Handback — convergence-human-repair` |
| `stop-at T-NN` reached and re-proven by the checker | STOP — return the receipt-backed decision |
| Runtime step or turn limit interrupts a phase | RECOVER — write/refresh `Interrupted Phase — T-NN / PHASE` |
| Fresh handback receipt returns `goal-verified` | STOP — return the receipt-backed terminal decision; do not run `/gal finalize` |
