# Task Atomicity

Canonical rules for splitting plan work into atomic `T-NN` tasks. Promoted from repeated planning-review lessons so every runtime and golem applies the same bar. The operational rubric and the quantitative split triggers live in the `/refining-plan` command contract; this file is the durable, cross-repo source of truth for the **anti-patterns** and the **atomicity principle**.

## Atomicity Principle

A task is atomic when ALL hold: (a) one logical change, (b) one rollback unit, (c) single-file / single-crate blast radius when feasible, (d) one reproducible acceptance probe, (e) it is *Haiku-executable* — a Haiku-tier model can one-shot it from the spec + named pointers, no extra judgment (a sizing heuristic, not a model-tier gate). See `commands/refining-plan` for the full rubric and the quantitative split triggers.

The natural atomic unit is a **single file / single crate / single concept — never a single line**. Thresholds are discussion triggers, not hard upper bounds; do not over-fragment a coherent change into micro-tasks.

## Anti-Patterns (split before locking `## Tasks`)

These are real, recurring decomposition failures observed in this repo's planning history. If a drafted task matches one, split it before the plan is locked.

- **Cross-target extraction hidden inside a move** — a "move X to Y" task that also silently extracts or refactors shared logic. Split the extraction from the move. *(e.g. a cross-crate move that buried a shared-logic extraction → had to be re-decomposed 14→20 tasks mid-pipeline.)*
- **rename + move bundled** — renaming a symbol and relocating it across modules in one task. Decouple rename from move. *(e.g. a field rename bundled with its relocation, only untangled at refining.)*
- **N-file sweep as one task** — "update all call sites" / "fix every occurrence" as a single task. Split per natural unit (per file or per crate). *(e.g. a single sweep = 351 hits / 69 files / 9 crates → had to become per-crate tasks.)*
- **Two big files in one task** — a single task that substantively edits two large files. One substantive file per task.
- **Split by target-location, not by atomic change** — tasks carved by where the code ends up instead of by one logical change. Carve by the change, not the destination.

## Plan-Size Bound

A single execution plan holds **≤ 99 blocking `T-NN` tasks**. Over ~80, warn; over 99, split the plan. This is the plan-level analogue of the per-task split triggers (WBS work-package sizing).

## When Atomicity Cannot Be Reached

If a task cannot be made atomic because it carries an undecided judgment (an architectural trade-off or design decision), that judgment is not an implementation task. Stop-line: return to `/deep-planning` to decide it first, and leave only the mechanical execution in the task.
