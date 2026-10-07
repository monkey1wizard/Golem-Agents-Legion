# Task Atomicity

Canonical rules for splitting plan work into atomic `T-NN` tasks. Promoted from repeated planning-review lessons so every runtime and golem applies the same bar. The operational rubric and the quantitative split triggers live in the `/refining-plan` command contract; this file is the durable, cross-repo source of truth for the **anti-patterns** and the **atomicity principle**.

## Atomicity Principle

The atomic unit is **one deliverable behavior forming one reviewable commit**. A task is atomic when it delivers one behavior, forms one rollback unit, has one reproducible acceptance probe, and carries no undecided judgment. File count and crate count do not define the unit. See `commands/refining-plan` for the Quantitative Split Triggers.

## Merge Triggers

These changes MUST be one task, even when they span several files:

- the same concept applied across several files
- the same change applied to every language variant of one document
- a change together with its own tests and docs

The Quantitative Split Triggers in `commands/refining-plan` stay as the upper bound. A merged task still splits when it crosses one of them.

## Verification-Only Tasks Are Forbidden

A task that only runs tests, lints, or checks and changes no behavior is not a task. Verification belongs to the `## Test Plan` and to goal-backward verification.

## Judgment Is Not a Task

If a task needs an undecided judgment (an architectural trade-off or design decision), that judgment is not an implementation task. Return to `/deep-planning` to decide it, and leave only the mechanical execution in the task. Readiness to execute never justifies a smaller task.

## Anti-Patterns (split before locking `## Tasks`)

These are real, recurring decomposition failures observed in this repo's planning history. If a drafted task matches one, split it before the plan is locked.

- **Cross-target extraction hidden inside a move** — a "move X to Y" task that also silently extracts or refactors shared logic. Split the extraction from the move. *(e.g. a cross-crate move that buried a shared-logic extraction → had to be re-decomposed 14→20 tasks mid-pipeline.)*
- **rename + move bundled** — renaming a symbol and relocating it across modules in one task. Decouple rename from move. *(e.g. a field rename bundled with its relocation, only untangled at refining.)*
- **Split by target-location, not by atomic change** — tasks carved by where the code ends up instead of by one logical change. Carve by the change, not the destination.

## Plan-Size Bound

A single execution plan holds **≤ 99 blocking `T-NN` tasks**. Over ~80, warn; over 99, split the plan. This is the plan-level analogue of the per-task split triggers (WBS work-package sizing).
