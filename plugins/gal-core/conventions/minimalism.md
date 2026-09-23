# Minimalism Ladder

Canonical source for the minimalism check applied at `/planning` start and at every adversarial review gate. Commands and agents reference this file pointer-only; they do not restate the ladder.

## Why

Models trend toward over-design. The ladder is a forced stop before any new mechanism, abstraction, or file is accepted. Each rung eliminates the proposal at the cheapest possible cost. Only if all rungs fail does the work proceed.

## Understanding Precedes Minimization

Read the actual code before running the ladder. Minimization applied to a misunderstood system removes the wrong things. Understanding first is what makes the ladder's convergence *feasibility-anchored* rather than abstract — you cannot answer "already in this codebase?" without having looked.

Three bounds keep this from becoming an excuse to re-read everything:

- **Depth proportional to change.** An obvious local fix needs only the touched code; a structural change needs the affected flow. The per-stage depth (how much to read at `/planning` vs `/refining-plan` vs review) is defined by the planning-order convention, not restated here.
- **Code is truth, docs are advisory.** When code and documentation disagree, the running code is the ground truth. Read the code to confirm behavior; treat docs as a hint, not a contract.
- **Aligned with directed exploration.** Follow `token-budget.md` directed-exploration: read only the files the current step requires, stop when the question is answered. Understanding-first is targeted reading, never a whole-codebase sweep.

## The Ladder

Ask these in order, stopping at the first YES:

1. **Does this need to exist?**
   → No clear user-visible or contract-visible need: **SKIP** (YAGNI). Record the rejected item in `## Open Questions` if there is genuine uncertainty.

2. **Already in this codebase?**
   → An existing function, module, type, or pattern already does it (or nearly does it): **reuse it, don't rewrite**. Extend or call the existing code instead of adding a parallel implementation.

3. **Does the standard library do it?**
   → Yes: **use it**, no new code.

4. **Does a native platform feature do it?**
   → Yes: **use it**, no new code.

5. **Does an already-installed dependency do it?**
   → Yes: **use it**, no new code.

6. **Can one line solve it?**
   → Yes: **write one line**.

7. **None of the above?**
   → Write the **minimum implementation that works**. No speculative abstractions, no future-proofing hooks, no helper that has a single caller.

## Scope Boundary

Minimalism acts on **feature scope and verbosity** — it removes unneeded capability, speculative abstraction, dead options, and wordiness. That is the whole of what it governs.

Minimalism does **not** act on **robustness**. Security, data-loss avoidance, trust-boundary enforcement, and accessibility are *not* candidates for the "does this need to exist?" cut — a hardening measure is not over-design just because it adds code. This is a definitional boundary that tells the ladder what it may and may not prune; it is **not** a security check and does **not** duplicate the auditor's downstream deep security/performance audit. When a proposal is a robustness measure, minimalism steps aside and the auditor owns the judgment.

## Mandatory Activation Points

This ladder **must** run at:

- **`/planning` start** — after understanding the actual code and a brief option expansion, challenge every proposed element before the plan is written.
- **Every adversarial review** — `/deep-planning` architect + steward reviews and `/refining-plan` STAGE 3.5 dual-lens both apply the ladder to what was proposed.

Failing to run the ladder before an adversarial review is itself a finding the reviewer must raise.
