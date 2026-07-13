---
name: adversarial-review
description: Single-source adversarial review method for plans, diffs, docs, decisions, and claims: steel-man first, refute by default under doubt, enforce evidence discipline, use APPROVE/REVISE/REJECT verdicts, stop at jidoka boundaries, and never treat NotRun as pass.
---

# Adversarial Review

Single-source method for adversarial review across GAL roles. This skill is target-agnostic: apply it to plans, diffs, docs, decisions, claims, and review write-back.

## Core Method

1. Steel-man the proposal first.
   Restate the strongest honest version of the proposal before attacking it. Do not argue against a weaker straw-man.
2. Refute by default under doubt.
   If a claim is under-evidenced, treat it as refuted until evidence arrives. Missing evidence is not a soft pass.
3. Run the minimalism gate by pointer, not restatement.
   Apply [conventions/minimalism.md](../../conventions/minimalism.md) before accepting any new mechanism, abstraction, file, or dependency. Do not restate the ladder here.
4. Enforce evidence discipline.
   Anchor findings in concrete repo evidence, observable behavior, deterministic receipts, or an explicitly named absence of evidence. Do not invent certainty.
5. Use explicit verdict vocabulary.
   - `APPROVE` = proceed
   - `REVISE` = fixable gaps; stop and loop back
   - `REJECT` = wrong direction; rethink from first principles
6. Treat `REVISE` as a jidoka stop-line.
   Do not continue downstream while a blocking review gap remains open.
7. `NotRun` is never pass.
   Missing execution, missing receipt, timeout, disconnected-partial, or unobserved behavior cannot be upgraded by optimistic prose.

## Anti-Patterns

- `Rubber stamping` — approving without genuine adversarial analysis
- `Bike-shedding` — spending review energy on trivia while missing structural risk
- `Pessimism theater` — declaring everything wrong without a better alternative
- `Abstractionitis` — demanding flexibility or indirection without evidence it is needed now

## Output Discipline

- Name the concrete trade-off or risk.
- Say what evidence supports it.
- Give the smallest viable alternative when you reject the current approach.
- Keep the method separate from the role lens: this skill supplies review discipline, while each role keeps its own domain dimensions.
