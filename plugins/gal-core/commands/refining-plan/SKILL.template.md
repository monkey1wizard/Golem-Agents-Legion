---
name: refining-plan
description: "GAL refining-plan ($refining-plan / /refining-plan / refine plan / 細化計劃 / lock tasks). Locks the implementation contract: writes ## Tasks (T-NN), ## Test Plan (TP-NN), ENG_REVIEW: CLEAR. Must have ≥1 blocking task. Runs gal refining-check receipt. Output enables $plan-to-prompt."
---

# /refining-plan

Lock the implementation contract for a reviewed source plan before human approval and prompt generation.

## Role

Engineering-review writer. Your job is to read the source plan, derive a concrete task breakdown and test matrix, record your verdict in the Engineering Review section, and emit the CLEAR marker when the plan is sound.

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

### Step 1a — Non-English Reconcile Preflight (on entry)

If the located source plan carries a planning-authority metadata block (`planLanguage != en`, the EN-draft flow — see `workflows/coding.md` → Planning-Language Authority): on entry, compute the localized rendered-source hash and compare it to the metadata. **On mismatch, STOP at a read-only reconcile preflight before writing any tasks** — the human hand-edited the localized source, so reconcile the divergence back into the EN draft (diff-base = `render(EN draft)`), re-render, and re-run `gal planning-stamp` first. Only once the hash matches may refining proceed.

Then lock `## Tasks` / `## Test Plan` using the **EN semantic draft as the semantic source** (it holds the authoritative technical meaning); the localized source stays human-readable in `planLanguage`, with machine anchors (task/test IDs, file paths, verdict literals) kept English (no rename). For `en`-prefix planLanguage there is no draft — refine the single-file source plan directly.

## Step 2 — Read and Evaluate the Plan

Read the full source plan. Focus on:

- `## Goal` — what outcome must be achieved
- `## Approach` — how the work is structured
- `## Requirements` / `## Files to Create or Modify` — scope of change
- Existing `## Review Results`, `## Open Questions`, `## Risks`, and `## Success Criteria`

Identify implementation tasks (T-NN), a test matrix, and any blockers.

**OQ-completion gate (precondition):** if `## Open Questions` still has any unresolved entry, **refuse to lock** — do not write `## Tasks`. **Refining has ZERO closure authority** — it never resolves, defaults, or demotes an OQ in order to proceed; if you are tempted to "resolve" one to keep going, *that is the exact bug this gate exists to stop*. Return to `/deep-planning` (or a direct `/gal architect` write-back review) to close every OQ first, by authority class (canonical rule: [`conventions/open-questions.md`](../../conventions/open-questions.md) — architect closes A/F with recorded rationale, human-only closes H, doubt → H). Refining may only proceed when `## Open Questions` is empty / `None`.

## Step 3 — Write ## Tasks

Overwrite the placeholder in the source plan `## Tasks` with numbered tasks:

```text
- [ ] T-NN — <short imperative description>
- [ ] T-NN — <short imperative description>
```

Each task must be independently completable and testable.

### Atomicity Rubric

A task is **atomic** only when ALL of the following hold:

- **(a) one logical change** — a single conceptual edit, not a bundle of unrelated edits.
- **(b) one rollback unit** — reverting the task's commit cleanly undoes exactly this change.
- **(c) single-file / single-crate blast radius when feasible** — prefer one file or one crate; cross-file is allowed only when the change is genuinely one concept.
- **(d) one reproducible acceptance probe** — a single observable check (grep for a named section, a `cargo test`, a CLI output) decides pass/fail.
- **(e) Haiku-executable (heuristic, not a mechanical gate)** — a Haiku-tier model can one-shot the task from the task spec + the named pointers (file paths, signatures, call sites) alone, with no extra judgment, and pass probe (d).

**(e) stop-line nuance** — if a task is NOT Haiku-executable:

- because it is too large / bundled → **split** it (see the quantitative split triggers below);
- because it carries an **undecided judgment** (architectural trade-off / design decision) → that judgment is not an implementation task. **Stop-line: return to `/deep-planning`** to decide it first; leave only the mechanical execution in the task.

Treat (e) as a sizing/readiness heuristic, not a literal "which model is Haiku" gate — do not spin on model-tier classification.

### Anti-Patterns (non-atomic task smells — split before locking)

- **Cross-target extraction hidden inside a move** — "move file X to Y" that also silently extracts/refactors shared logic. Split the extraction from the move.
- **rename + move bundled** — renaming a symbol and relocating it across modules in one task. Decouple rename from move.
- **N-file sweep as one task** — "update all call sites" / "fix every occurrence" as a single task. Split per natural unit (per file or per crate).
- **Two big files in one task** — a single task that substantively edits two large files. One substantive file per task.
- **Split by target-location, not by atomic change** — tasks carved by "where the code ends up" instead of "one logical change". Carve by the change, not the destination.

### Quantitative Split Triggers

Before locking `## Tasks`, measure each task's blast radius (grep / file count). A task **must be split** when it crosses any threshold:

- touches **more than ~10 files**, or
- spans **more than 1 crate**, or
- bundles **rename + move + content change** together, or
- covers **more than 1 concept**.

Record the measured number in the task or in the Review when a task approaches a threshold, so over-coarse tasks are visible.

**Over-fragmentation guard:** the natural atomic unit is a **single file / single crate / single concept — never a single line**. Thresholds are *discussion triggers*, not hard upper bounds; the Atomicity Rubric + the STAGE 3.5 gate make the final call. Do not shatter one coherent change into micro-tasks.

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

Overwrite the placeholder in the source plan `## Test Plan` with a matrix aligned to the T-NN tasks above:

```text
| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-NN | unit | ... | T-NN |
```

Include unit, integration, and manual test entries as appropriate.
Each test-plan row should validate the acceptance condition stated in its matching `T-NN` task, so an implementer or tester can verify the task from the task spec alone instead of reconstructing missing context from the full plan.
Where a task's acceptance depends on dispatch evidence, the matching test-plan row must check both the focused behavior and the named evidence shape instead of treating an unverified PASS claim as sufficient.
When the task adds or renames `crates/gal-engine/tests/*.rs` files, avoid `install`, `setup`, `update`, and `patch` in the filename unless the crate already ships an `asInvoker` manifest, and never treat a spawn failure such as Windows error 740 as exit 0 just because the command ran through a pipe.

## Step 5 — STAGE 3.5 Closeout Adversarial Gate (ENG_REVIEW Precondition)

**`<!-- ENG_REVIEW: CLEAR -->` may only be emitted after this gate passes.** This is the STAGE 3.5 REFINE-LOCK dual-lens from `workflows/coding.md` wired into the refining SKILL body as a hard precondition — it is not a new concept, just the existing gate applied here.

**Order:** refining is the converge-stage application of the shared **Planning Order Principle** ([`workflows/coding.md`](../../workflows/coding.md) → Planning Reviews) — understand → diverge → converge; this gate is where the plan finally converges to a locked, minimal task set (pointer-only; the principle is not restated here).

**Depth-scaled:** structural/multi-file/Protected-Path changes require both lenses. Obvious local-fix plans may collapse to a self-check against the rubric with a brief one-paragraph verdict.

### Lens 1 — Architect: Structural Atomicity

Apply the structural-atomicity lens (ref `workflows/coding.md` STAGE 3.5 and `conventions/task-atomicity.md`):

- Are all tasks atomic? (single logical change, single rollback, single-file/crate where feasible)
- No rename+move+content bundled in one task?
- No N-file sweep hidden as one task?
- Blast radius measured and within split triggers?
- All tasks Haiku-executable from their spec alone?
- Minimalism check (ref `conventions/minimalism.md`): does each task do less than the minimum? If any task adds speculative scope beyond the plan requirements, flag REVISE.
- Refuse to emit `ENG_REVIEW: CLEAR` while an applicable item from `conventions/task-quality.md` has no answer in the task's `Change` and `Acceptance`.

Verdict: **APPROVE** or **REVISE** (with specific tasks to split or descope). Any REVISE → do not proceed; return to Step 3 to fix.

### Lens 2 — Tester: Minimum Observable Probe

Apply the tester lens:

- Does every T-NN task have a `TP-NN` test-plan entry with a minimal, reproducible, observable acceptance probe?
- Is the probe end-to-end observable (not just "exit 0" — an observable write-back, grep, or behavioral signal)? Ref `workflows/coding.md` honest-pass-bar: exit 0 alone ≠ PASS.
- Does the test contract specify the evidence shape (file written, output contains, binary exit) the orchestrator can verify without trusting a self-report?

Verdict: **APPROVE** or **REVISE** (with specific TP-NN entries to strengthen). Any REVISE → do not proceed; return to Step 4 to fix.

### Diagram Sync Check

Confirm the plan's `## Diagrams` section (if present) is in sync with the finalized `## Tasks` scope. If the diagram is now stale after task refinement, update it before emitting CLEAR. Depth-scaled: no diagram for local-fix plans.

### Emit Verdict

Only when **both** Lens 1 and Lens 2 return APPROVE and any diagram is in sync:

Run:

```powershell
gal refining-check .dev/plans/<plan-slug>.md
```

Read the receipt. `<!-- ENG_REVIEW: CLEAR -->` may be emitted only when the receipt is `pass`.
The default evidence path is `.dev/pipeline/<plan-scope-key>/refining-check.receipt.md`.

Overwrite the source plan `## Review Results > ### Engineering Review`:

- Write verdict **CLEAR** with a brief rationale (atomicity check + probe check summary), then append `<!-- ENG_REVIEW: CLEAR -->` on its own line at the end of the `### Engineering Review` section.

If either lens returns REVISE or any blocker remains:

- Write verdict **BLOCKING**, list each REVISE finding per lens, and return to the blocking step (Step 3 for atomicity, Step 4 for test plan). Do not emit `<!-- ENG_REVIEW: CLEAR -->`.

## Scope Guard

This command only populates the source plan `## Tasks`, `## Test Plan`, and `## Review Results > ### Engineering Review`. It must not implement any code, run tests, modify `.dev/plans/`, or change `## Status`.
