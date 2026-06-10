---
name: refining-plan
description: "Lock the implementation contract for a source plan: populate ## Tasks, ## Test Plan, and ## Review Results > ### Engineering Review, and emit <!-- ENG_REVIEW: CLEAR --> when the plan passes."
---

# /refining-plan

Lock the implementation contract for a reviewed source plan before prompt generation.

## Role

Engineering-review writer. Your job is to read the source plan, derive a concrete task breakdown and test matrix, record your verdict in the Engineering Review section, and emit the CLEAR marker when the plan is sound.

## When to Use

- `docs/plans/<slug>.md` exists and still has placeholder `## Tasks`, `## Test Plan`, or `### Engineering Review`
- A reviewed source plan needs an implementation contract before `/plan-to-prompt` generates the execution prompt
- An existing execution prompt has gone stale and needs to be refreshed from a corrected source plan

## Step 1 — Locate the Source Plan

If the user specified a slug, use `docs/plans/<slug>.md`.

Otherwise:

1. Read `.dev/state.md` `## Active Plans`.
2. If the active file already points at `docs/plans/<slug>.md`, use it.
3. If the active file points at `.dev/plans/<slug>.prompt.md`, resolve the matching source plan `docs/plans/<slug>.md`.
4. If multiple candidate source plans still need engineering review, ask the user which one to refine.

## Step 2 — Read and Evaluate the Plan

Read the full source plan. Focus on:

- `## Goal` — what outcome must be achieved
- `## Approach` — how the work is structured
- `## Requirements` / `## Files to Create or Modify` — scope of change
- Existing `## Review Results`, `## Open Questions`, `## Risks`, and `## Success Criteria`

Identify implementation tasks (T-NNN), a test matrix, and any blockers.

## Step 3 — Write ## Tasks

Overwrite the placeholder in the source plan `## Tasks` with numbered tasks:

```text
- [ ] T-001 — <short imperative description>
- [ ] T-002 — <short imperative description>
```

Each task must be independently completable and testable.

Each `T-NNN` task must be a self-contained execution unit that includes:

- exact target file path(s)
- the concrete change to make in those files
- an in-place acceptance check that can be run locally against that task
- the focused probe and evidence shape that prove the task really passed
- any required convention, signature, dependency, or call-site pointers needed to complete the task safely

Keep the task pointer-style and budget-bounded:

- Do **not** embed full file contents in the task body. The executor reads the named files itself.
- Target a `<5KB`-class instruction + pointer budget per task. The limit exists to keep the task focused and cheap to dispatch, not because of model context-window size.
- If a task cannot stay within that budget while remaining self-contained, split it into smaller atomic tasks before finalizing `## Tasks`.
- When the task is expected to run through headless dispatch, name the evidence as an executor-log terminal state of `completed` plus the observable write-back pointer the orchestrator should verify. When the run is intentionally `DEGRADED_BUNDLED`, name the reproducible command output or task-scoped write-back evidence instead of pretending an executor log exists.

## Step 4 — Write ## Test Plan

Overwrite the placeholder in the source plan `## Test Plan` with a matrix aligned to the T-NNN tasks above:

```text
| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | unit | ... | T-001 |
```

Include unit, integration, and manual test entries as appropriate.
Each test-plan row should validate the acceptance condition stated in its matching `T-NNN` task, so an implementer or tester can verify the task from the task spec alone instead of reconstructing missing context from the full plan.
Where a task's acceptance depends on dispatch evidence, the matching test-plan row must check both the focused behavior and the named evidence shape instead of treating an unverified PASS claim as sufficient.
When the task adds or renames `crates/gal-engine/tests/*.rs` files, avoid `install`, `setup`, `update`, and `patch` in the filename unless the crate already ships an `asInvoker` manifest, and never treat a spawn failure such as Windows error 740 as exit 0 just because the command ran through a pipe.

## Step 5 — Write Engineering Review Verdict

Overwrite the source plan `## Review Results > ### Engineering Review`:

- If the plan is sound: write verdict **CLEAR** with a brief rationale, then append `<!-- ENG_REVIEW: CLEAR -->` on its own line at the end of the `### Engineering Review` section.
- If the plan has blockers: write verdict **BLOCKING**, list each blocker, and recommend returning to `/deep-planning` to resolve them before proceeding to `/plan-to-prompt`.

## Scope Guard

This command only populates the source plan `## Tasks`, `## Test Plan`, and `## Review Results > ### Engineering Review`. It must not implement any code, run tests, modify `.dev/plans/`, or change `## Status`.
