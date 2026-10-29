---
name: refining-plan
description: "GAL refining-plan ($refining-plan / /refining-plan / refine plan / 細化計劃 / lock tasks). Can produce the implementation contract: ## Tasks (T-NN), ## Test Plan (TP-NN), and ENG_REVIEW: CLEAR. Must have ≥1 blocking task. Runs gal refining-check receipt. A passing gate enables $plan-to-prompt regardless of producer."
---

# /refining-plan

Lock the implementation contract for a reviewed source plan before human approval and prompt generation.

## Role

One available implementation-contract producer. Read the source plan, derive a concrete task breakdown and test matrix using `plugins/gal-core/templates/plan.md`, record the STAGE 3.5 dual-lens verdict in the Engineering Review section, and emit the CLEAR marker when the plan is sound.

## When to Use

- `.dev/plans/<slug>.md` exists and still has placeholder `## Tasks`, `## Test Plan`, or `### Engineering Review`
- A reviewed source plan needs an implementation contract before human approval and `/plan-to-prompt`
- An existing execution prompt has gone stale and needs to be refreshed from a corrected source plan

## Step 1 — Locate the Source Plan

If the user specified a slug, use `.dev/plans/<slug>.md`.

Otherwise:

1. Read `.dev/state.md` `## Active Plans`.
2. If the active file already points at `.dev/plans/<slug>.md`, use it.
3. If the active file points at `.dev/plans/<slug>.prompt.md`, resolve the matching source plan `.dev/plans/<slug>.md`.
4. If multiple candidate source plans still need engineering review, ask the user which one to refine.

### Step 1a — Non-English Source Plan

For `planLanguage != en`, lock `## Tasks` / `## Test Plan` directly in the single source plan. Keep narrative prose in `planLanguage` and machine anchors, including task and test IDs, file paths, and verdict literals, in English. Do not create or update an EN draft, reconcile hashes, render a paired file, or run `gal planning-stamp` for new plans. Legacy plans with a `gal:planning-authority` block remain subject to localized-metadata validation; preserve that metadata while editing.

## Step 2 — Read and Evaluate the Plan

Before evaluating the plan, read `.dev/mistake-register.md`. If the register is missing, create it from `plugins/gal-core/templates/mistake-register.md`, then read it. Use its evidence-backed corrections when assessing the plan. Treat an unstated cause as unknown, and do not turn a register entry into a new requirement unless the plan's scope supports it.

Read the full source plan. Focus on:

- `## Goal` — what outcome must be achieved
- `## Approach` — how the work is structured
- `## Requirements` / `## Files to Create or Modify` — scope of change
- Existing `## Review Results`, `## Open Questions`, `## Risks`, and `## Success Criteria`

Identify implementation tasks (T-NN), a test matrix, and any blockers.

### Ground the task contract

Before drafting or locking tasks, read the relevant changed code and direct
dependencies. Cite every task statement about existing behavior with the real
`file:line` that supports it. In each applicable task, describe the current
behavior and the behavior the change will deliver. Prove one concrete safety
fact by running the relevant existing code when that is cheap. If it cannot be
proven that way, mark the fact `unproven` in the task instead of presenting it
as established. Identify every existing guarantee the change removes or
weakens, then state how the task keeps each guarantee or deliberately drops it.
If the code or approved plan cannot establish a required decision, return the
gap for decision instead of assuming an answer.

Name each relevant existing read-only baseline command, the evidence it
produces, and the agent or command responsible for producing and recording that
evidence. A baseline is evidence about the current state. It never authorizes
feature edits, new test code, or closure of an Open Question. Complete each
task's file list, explain each file's role, and assign concerns shared across
tasks to one named task. Apply the four grounding questions in
[`conventions/task-quality.md`](../../conventions/task-quality.md) and the
completeness and divergent-implementer rules in
[`conventions/task-atomicity.md`](../../conventions/task-atomicity.md).

For any task producer's change to shared traits, CLI signatures, or templates,
include a Blast-Radius Consumer Matrix in the task contract. List direct and
indirect consumers by file or interface, describe the impact, and name the task
that updates each consumer or provide compatibility evidence. Apply this rule
regardless of which command or workflow produced the task. Do not create a
permanent hand-maintained consumer registry.

For every existing acceptance command named in `## Tasks`, run it once against
the base revision in read-only mode and record the command, result, and evidence
in `### Engineering Review`. If an acceptance suite already fails at the base,
state its pass rule relative to that recorded baseline: the change must
introduce no new failures against the recorded set.

Every task's `Acceptance` text and each covering `TP-NN` row must name where
its evidence is produced: the implementer executor host, the dispatched tester
inputs, or a named CI job. Move evidence that none of these can produce to
`## Owner Acceptance` or a `## Preconditions` row before lock. Do not make an
acceptance condition depend on behavior that a later task introduces.

**OQ-completion gate (precondition):** if `## Open Questions` still has any unresolved entry, **refuse to lock** — do not write `## Tasks`. **Refining has ZERO closure authority** — it never resolves, defaults, or demotes an OQ in order to proceed; if you are tempted to "resolve" one to keep going, *that is the exact bug this gate exists to stop*. Return to `/deep-planning` (or a direct `/gal architect` write-back review) to close every OQ first, by authority class (canonical rule: [`conventions/open-questions.md`](../../conventions/open-questions.md) — architect closes A/F with recorded rationale, human-only closes H, doubt → H). Refining may only proceed when `## Open Questions` is empty / `None`.

## Step 3 — Write ## Tasks

When `## Tasks` is a placeholder, replace it with numbered tasks using the task shape in `plugins/gal-core/templates/plan.md`:

```text
- [ ] T-NN — <short imperative description>
- [ ] T-NN — <short imperative description>
```

Each task must be independently completable and testable.

### Atomicity and Merge Rules

Apply the atomicity principle, merge triggers, verification-only ban, and anti-patterns in [`conventions/task-atomicity.md`](../../conventions/task-atomicity.md). That convention defines one deliverable behavior forming one reviewable commit as the atomic unit. Treat the "Haiku-executable" heuristic only as a flag for undecided judgment. Return undecided judgment to `/deep-planning`; do not split a task only to make execution easier.

### Anti-Patterns (non-atomic task smells — split before locking)

- **Cross-target extraction hidden inside a move** — "move file X to Y" that also silently extracts/refactors shared logic. Split the extraction from the move.
- **rename + move bundled** — renaming a symbol and relocating it across modules in one task. Decouple rename from move.
- **Split by target-location, not by atomic change** — tasks carved by "where the code ends up" instead of "one logical change". Carve by the change, not the destination.

### Quantitative Split Triggers

Apply the canonical quantitative split triggers in [`conventions/task-atomicity.md`](../../conventions/task-atomicity.md#quantitative-split-triggers).

**Plan-size bound:** a single execution plan holds **≤ 99 blocking `T-NN` tasks**. Over ~80, warn; **over 99, refuse to lock — split the plan** (this is the plan-level analogue of the per-task split triggers; the STAGE 3.5 gate enforces it). See `conventions/task-atomicity.md`.

**Minimum-task guard:** a plan with zero `T-NN` tasks in `## Tasks` is not a plan — it is a scope statement. **Refuse to emit `ENG_REVIEW: CLEAR`** when `## Tasks` has no blocking tasks. `gal refining-check` enforces this mechanically (zero tasks → `task-count: fail`).

### Test Contract (spec) ≠ Test Code

At refining / STAGE 3.5 you define the **test contract**, not test code:

| Stage | Output | Form |
| --- | --- | --- |
| refining / 3.5 (planning) | the `TP-NN` oracle: behavior to observe, evidence shape, minimal scope | prose / one-line spec — NO `assert`, fixture, or test fn |
| implementation (pipeline) | the actual runnable test code | real code, written by `golem-tester` (≠ implementer model) from spec + public API |

This is ATDD / Specification-by-Example / BDD ("define acceptance first, code later") — **not** "write the unit tests early". The planning stage emits no test code, so the implementer never sees pre-written tests (independence preserved).

**~1-line probe guard:** every task's acceptance probe must be describable in ~1 line of spec. **If you cannot state the probe without first writing real test code, that is a "not-ready" signal** — split the task or return to `/deep-planning`. It is NOT a license to start writing a test suite inside refining.

Each `T-NN` task must be a self-contained execution unit that includes:

- exact target file path(s)
- the concrete change to make in those files
- an in-place acceptance check that can be run locally against that task
- the focused probe and evidence shape that prove the task really passed
- any required convention, signature, dependency, or call-site pointers needed to complete the task safely
- the applicable items of `conventions/task-quality.md` are answered inside `Change` and `Acceptance`; items that do not apply are not mentioned

Keep the task pointer-style and budget-bounded:

- Do **not** embed full file contents in the task body. The executor reads the named files itself.
- Target a `<5KB`-class instruction + pointer budget per task. The limit exists to keep the task focused and cheap to dispatch, not because of model context-window size.
- If a task cannot stay within that budget while remaining self-contained, split it into smaller atomic tasks before finalizing `## Tasks`.
- When the task is expected to run through headless dispatch, name the evidence as an executor-log terminal state of `completed` plus the observable write-back pointer the orchestrator should verify. When the run is intentionally `DEGRADED_BUNDLED`, name the reproducible command output or task-scoped write-back evidence instead of pretending an executor log exists.
- Make each task self-cleaning: if the change would orphan dead helpers, stale tests, or copied caveats, the same task must remove or refresh them before it can be called done. The orchestrator and auditor should treat leftover residue as a task failure, not a follow-up nicety.

## Step 4 — Write ## Test Plan

Overwrite the placeholder in the source plan `## Test Plan` with a matrix of behaviors:

```text
| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-NN | unit | ... | T-NN, T-NN |
```

Write one `TP-NN` row per behavior. A row's `Covers` cell may name several tasks, and TP numbers need not equal task numbers. Every `T-NN` task must appear in the `Covers` cell of at least one row.
Every row is executed by an agent. A check that needs human judgment or a human action does not belong in the Test Plan. Put it in `## Owner Acceptance`.
Use the `manual` type only for an agent-run ad-hoc probe whose output the agent records. A plan holds at most one `manual` row. Its Description carries the literal label `Why manual:` followed by the reason no `unit`, `integration`, `grep`, or `documentation` row fits.
Each row must validate the acceptance condition of the tasks it covers, so an implementer or tester can verify them from the task spec alone instead of reconstructing missing context from the full plan.
Where a task's acceptance depends on dispatch evidence, the covering test-plan row must check both the focused behavior and the named evidence shape instead of treating an unverified PASS claim as sufficient.
When the task adds or renames `crates/gal-engine/tests/*.rs` files, avoid `install`, `setup`, `update`, and `patch` in the filename unless the crate already ships an `asInvoker` manifest, and never treat a spawn failure such as Windows error 740 as exit 0 just because the command ran through a pipe.

## Step 4a — Write the Optional ## Preconditions and ## Owner Acceptance

Both sections are optional. A plan with neither section is the preferred shape. Human review is never a `T-NN` task and never a stop between tasks.

**`## Owner Acceptance`** holds checks that need human judgment or a human action. Write at most 3 items, one row per item, with these columns and `OA-NN` IDs:

```text
| ID | What to check | Expected result | Pass/fail rule | Why not automated |
| --- | --- | --- | --- | --- |
| OA-NN | `<exact path, command, or URL>` | what the owner sees when it works | one observable criterion | one sentence |
```

**`## Preconditions`** holds environment needs only the owner can meet. Use `PC-NN` IDs and these columns. The orchestrator runs every `Check command` before `gal pipeline-preflight`:

```text
| ID | Requirement | Check command | Expected result | How to satisfy |
| --- | --- | --- | --- | --- |
| PC-NN | what must exist | `<command>` | output that proves it | how the owner fixes it |
```

Cell rules for both tables, enforced by `gal refining-check`:

- Every cell is non-empty and is not a placeholder (`TBD`, `TODO`, `pending`, `N/A`, `...`, `…`, or any `<...>` text).
- Every cell is at most 200 Unicode characters after backticked spans are removed.
- `What to check` (Owner Acceptance) and `Check command` (Preconditions) each contain at least one backticked path, command, or URL.

## Step 5 — STAGE 3.5 Closeout Adversarial Gate (ENG_REVIEW Precondition)

**`<!-- ENG_REVIEW: CLEAR -->` may only be emitted after this gate passes.** This is the STAGE 3.5 REFINE-LOCK dual-lens from `workflows/coding.md` wired into the refining SKILL body as a hard precondition — it is not a new concept, just the existing gate applied here.

**Order:** refining is the converge-stage application of the shared **Planning Order Principle** ([`workflows/coding.md`](../../workflows/coding.md) → Planning Reviews) — understand → diverge → converge; this gate is where the plan finally converges to a locked, minimal task set (pointer-only; the principle is not restated here).

**Depth-scaled:** for structural, multi-file, or Protected-Path plans, independently dispatch Lens 1 to `golem-architect` and Lens 2 to `golem-tester`. Record both verdicts in `### Engineering Review`. The refiner must not approve its own task set. Documentation tasks skip TESTER dispatch under the existing contract. Obvious local-fix plans may use an in-process self-check against the rubric with a brief one-paragraph verdict.

### Lens 1 — Architect: Structural Atomicity

Apply the structural-atomicity lens (ref `workflows/coding.md` STAGE 3.5 and `conventions/task-atomicity.md`):

- Are all tasks atomic under `conventions/task-atomicity.md`?
- No rename+move+content bundled in one task?
- Are the convention's merge triggers applied?
- Are there any verification-only tasks?
- Blast radius measured and within split triggers?
- No task carries an undecided judgment?
- Are the four grounding questions answered where applicable, with current-code citations, one safety fact, removed guarantees, and named baseline evidence producers?
- Is every task's file list complete, and is each shared concern assigned to one task?
- Would reasonable implementers reach the same behavior from the task text? If not, return the judgment to `/deep-planning`.
- Minimalism check (ref `conventions/minimalism.md`): does each task do less than the minimum? If any task adds speculative scope beyond the plan requirements, flag REVISE.
- Refuse to emit `ENG_REVIEW: CLEAR` while an applicable item from `conventions/task-quality.md` has no answer in the task's `Change` and `Acceptance`.

Verdict: **APPROVE** or **REVISE** (with specific tasks to split or descope). Any REVISE → do not proceed; return to Step 3 to fix.

### Lens 2 — Tester: Minimum Observable Probe

Apply the tester lens:

- Is every T-NN task covered by at least one `TP-NN` row, and does each row validate one behavior with a minimal, reproducible, observable acceptance probe?
- Is every row executable by an agent, with no human judgment or human action hidden in it?
- Does the plan hold at most one `manual` row, and does its Description state `Why manual:` with a reason?
- Does each `## Owner Acceptance` item let the owner decide pass or fail from its row alone, without a follow-up question? Check that `What to check` names the exact target, `Expected result` says what the owner sees, and `Pass/fail rule` is one observable criterion.
- Is the probe end-to-end observable (not just "exit 0" — an observable write-back, grep, or behavioral signal)? Ref `workflows/coding.md` honest-pass-bar: exit 0 alone ≠ PASS.
- Does the test contract specify the evidence shape (file written, output contains, binary exit) the orchestrator can verify without trusting a self-report?
- Does the task name who produces each required baseline and acceptance evidence, and does it require the implementer executor log to end in `completed`? Documentation tasks skip TESTER dispatch under the existing contract. Do not invent a tester receipt.
- For every existing acceptance command named in `## Tasks`, was it run once on the base in read-only mode, with its result recorded in `### Engineering Review`? Is any already-failing suite's acceptance rule expressed as no new failures against that baseline?
- Does every task `Acceptance` and covering `TP-NN` row name an evidence producer, and is evidence outside executor, dispatched tester inputs, or named CI routed to `## Owner Acceptance` or `## Preconditions` before lock? Does acceptance avoid behavior introduced only by a later task?

For every applicable `TP-NN`, the orchestrator records the writing-command
output and independent-agent comparison in `## Test Results`, including the TP
ID and outcome. Record checks that did not run as `NotRun`. Never invent a
tester receipt or report an unrun check as passed.

Verdict: **APPROVE** or **REVISE** (with specific TP-NN entries to strengthen). Any REVISE → do not proceed; return to Step 4 to fix.

### Diagram Sync Check

Confirm the plan's `## Diagrams` section (if present) is in sync with the finalized `## Tasks` scope. If the diagram is now stale after task refinement, update it before emitting CLEAR. Depth-scaled: no diagram for local-fix plans.

### Emit Verdict

Only when **both** Lens 1 and Lens 2 return APPROVE and any diagram is in sync, perform these steps in order:

1. Overwrite the source plan `## Review Results > ### Engineering Review` with verdict **CLEAR** and a brief atomicity and probe summary. Append `<!-- ENG_REVIEW: CLEAR -->` on its own line at the end of that section. The marker records the completed review. It does not replace a passing receipt or authorize handoff by itself.
2. For a non-English plan, make plan changes in the single source plan. Keep narrative prose in the plan language and machine anchors in English. Finish plan edits and required writing checks before producing the final receipt.
3. Run the following check with the guarded-entry executable:

```powershell
gal refining-check .dev/plans/<plan-slug>.md
```

The checker requires the marker as an input, so write the reviewed verdict before running it. Read the receipt and require `pass` before handoff.
The default evidence path is `.dev/pipeline/<plan-scope-key>/refining-check.receipt.md`.
Before trusting the stored pass, run the bound executable with `doctor --verify-receipt <receipt> --expect-scope standalone` and require exit 0. Follow [`conventions/self-bootstrap.md`](../../conventions/self-bootstrap.md) for explicit executable roots and package-manager ownership; never resolve `gal` again through `PATH` after handoff.

If this is the outer workflow invocation, initialize its stable `invocation_id` and consumed format-repair round count to `0`. Otherwise carry both forward without reset. For an eligible, bound `refining` receipt whose failed or not-run rows are all `format`, follow [Bounded Format Recovery](../../conventions/token-budget.md#bounded-format-recovery): invoke the helper with the receipt's exact target, receipt, repair, gate, mode, gate attempt, and repair round values, then rerun `gal refining-check` on the same source plan and verify its fresh standalone receipt. Catalogued recovery requires the source plan to be tracked, committed, clean, and have no staged entries because `gal format-repair` refuses other target states. If this precondition does not hold, stop and ask the owner to commit the source plan. Then rerun `gal refining-check` and apply recovery only to a fresh eligible receipt. This stop consumes no round. A document round consumes one count after the helper exits, whether it succeeds or fails. The shared ceiling is two rounds. For any eligible phase-payload write-back in this invocation, pass `--format-repair-context '{"invocation_id":"<invocation_id>","round":<next_round>}'` only to `gal pipeline <prompt>`, where `<next_round>` is the consumed count plus one. When both rounds are consumed, omit the option. Never pass it to executor raw arguments or `gal format-repair`. Payload rounds consume a count only when the runtime prints `FORMAT_REPAIR`. Do not edit the source plan after the helper commit because that would stale the format receipt. The helper stamps the target itself. Apply recovery before removing the marker or writing BLOCKING. If recovery does not apply or stops, remove the marker, write verdict **BLOCKING** with the observed failure, and return to the blocking step. Missing or ambiguous markers/content and stale or unbound evidence are not formatting defects and stop recovery. For an `owner` row or helper refusal/failure, follow [Bounded Format Recovery's stop report table](../../conventions/token-budget.md#stop-and-next-action). When both rounds are consumed, follow that table and do not repair again. Recovery cannot create approval, task content, OQ closure, or evidence. Do not treat the failed receipt as pass.

If execution stops after marker emission but before a verified passing receipt, the marker alone does not permit handoff. On resume, confirm that both review approvals still cover the current plan and repeat the check and receipt verification. Any later plan edit invalidates that handoff evidence and requires a fresh check and verification. Preserve failed receipts as evidence when rerunning.

If either lens returns REVISE or any blocker remains:

- Write verdict **BLOCKING**, list each REVISE finding per lens, and return to the blocking step (Step 3 for atomicity, Step 4 for test plan). Do not emit `<!-- ENG_REVIEW: CLEAR -->`.

## Scope Guard

This command may update placeholder source-plan `## Tasks`, `## Test Plan`, and `## Review Results > ### Engineering Review` sections. Any producer may complete these sections by following `plugins/gal-core/templates/plan.md` and the same STAGE 3.5 dual-lens review contract. It may run existing read-only baseline commands to record current-state evidence, including each existing acceptance command named in `## Tasks` once on the base. It must not implement feature code, write new test code, run newly authored feature tests, create or mutate an execution prompt, close Open Questions, or change `## Status`. A baseline result is evidence only and grants no authority to write feature code or resolve an owner decision. The planning and refining checks and receipt verification required above remain in scope.
