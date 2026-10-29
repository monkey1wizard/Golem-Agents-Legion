# Task Atomicity

Canonical rules for splitting plan work into atomic `T-NN` tasks. Promoted from repeated planning-review lessons so every runtime and golem applies the same bar. This file is the durable, cross-repo source of truth for the **anti-patterns**, the **atomicity principle**, and the **quantitative split triggers**. The `/refining-plan` command contract points here.

## Atomicity Principle

The atomic unit is **one deliverable behavior forming one reviewable commit**. A task is atomic when it delivers one behavior, forms one rollback unit, has one reproducible acceptance probe, and carries no undecided judgment. File count and crate count do not define the unit.

Before locking, list every file the task is expected to change and give each
file's role in the deliverable. Include directly affected tests and
documentation. For a visibility change, include its consumers. Include any
registry the change must update and every path the workflow itself writes. If
implementation may touch additional files, name the bounded set or state the
discovery rule that determines it. An incomplete file list is not a complete
task contract. Assign each concern shared by multiple tasks to one named task,
and have other tasks point to that owner instead of duplicating or leaving the
concern unowned.

## Merge Triggers

These changes MUST be one task, even when they span several files:

- the same concept applied across several files
- the same change applied to every language variant of one document
- a change together with its own tests and docs

## Quantitative Split Triggers

Before locking `## Tasks`, measure each task's blast radius (file count). A task **must be split** when it crosses any threshold:

- touches **more than ~10 files**, or
- spans **more than 1 crate**, or
- bundles **rename + move + content change** together, or
- covers **more than 1 concept**.

Record the measured number in the task or in the Review when a task approaches a threshold, so over-coarse tasks are visible.

**Upper bound:** these thresholds cap a merged task. Merge triggers decide when to combine, split triggers decide when a combined task is too large. The Atomicity Rubric and the readiness gate make the final call.

A merged task still splits when it crosses one of these triggers.

## Verification-Only Tasks Are Forbidden

A task that only runs tests, lints, or checks and changes no behavior is not a task. Verification belongs to the `## Test Plan` and to goal-backward verification.

## Judgment Is Not a Task

If a task needs an undecided judgment (an architectural trade-off or design decision), that judgment is not an implementation task. Return to `/deep-planning` to decide it, and leave only the mechanical execution in the task. Readiness to execute never justifies a smaller task.

If two implementers given the same `Change` text could produce different
accept or reject results for the same input, the judgment is undecided. Return
the plan to `/deep-planning` for a decision before locking. Refining never
fills this gap itself. Do not treat an implementer's ability to choose a
plausible option as evidence that the task is ready.

## Anti-Patterns (split before locking `## Tasks`)

These are real, recurring decomposition failures observed in this repo's planning history. If a drafted task matches one, split it before the plan is locked.

- **Cross-target extraction hidden inside a move** — a "move X to Y" task that also silently extracts or refactors shared logic. Split the extraction from the move. *(e.g. a cross-crate move that buried a shared-logic extraction → had to be re-decomposed 14→20 tasks mid-pipeline.)*
- **rename + move bundled** — renaming a symbol and relocating it across modules in one task. Decouple rename from move. *(e.g. a field rename bundled with its relocation, only untangled at refining.)*
- **Split by target-location, not by atomic change** — tasks carved by where the code ends up instead of by one logical change. Carve by the change, not the destination.

## Plan-Size Bound

A single execution plan holds **≤ 99 blocking `T-NN` tasks**. Over ~80, warn; over 99, split the plan. This is the plan-level analogue of the per-task split triggers (WBS work-package sizing).
