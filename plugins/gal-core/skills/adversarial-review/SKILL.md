---
name: adversarial-review
description: Single-source adversarial review method for plans, diffs, docs, decisions, and claims: steel-man first, refute by default under doubt, enforce evidence discipline, use APPROVE/REVISE/REJECT verdicts, stop at jidoka boundaries, and never treat NotRun as pass.
context: fork
background: false
---

# Adversarial Review

Single-source method for adversarial review across GAL roles. This skill is target-agnostic: apply it to plans, diffs, docs, decisions, claims, and review write-back.

## Forked Invocation

Direct invocation (`/adversarial-review <target>`) runs in an independent subagent with no access to the invoking conversation. That isolation is the point: a review inherited from the authoring context is not an independent judgment.

Review target: $ARGUMENTS

- Resolve any referenced path against the repository and read it yourself. Nothing about the target is already known to you.
- Gather evidence with your own tools before forming a verdict. A finding with no repo evidence, no observed behavior, and no named receipt is refuted by rule 2, including your own.
- Return one verdict block — `APPROVE` / `REVISE` / `REJECT` — with the evidence cited by file path and line.
- With no target given, return `REVISE: no review target provided`.

Roles that apply this method from their own agent context (architect, analyst, designer, tester, steward) read the sections below and ignore this one.

## Core Method

1. Steel-man the proposal first.
   Restate the strongest honest version of the proposal before attacking it. Do not argue against a weaker straw-man.
2. Refute by default under doubt.
   If a claim is under-evidenced, treat it as refuted until evidence arrives. Missing evidence is not a soft pass.
3. Run the minimalism gate by pointer, not restatement.
   Apply [conventions/minimalism.md](../../conventions/minimalism.md) before accepting any new mechanism, abstraction, file, or dependency. Do not restate the ladder here.
4. Enforce evidence discipline.
   Anchor findings in concrete repo evidence, observable behavior, deterministic receipts, or an explicitly named absence of evidence. Do not invent certainty.
   Grade claims that can change a design, acceptance, or review verdict against the claim's input, version, and code path:
   - Grade 1: An assertion without supporting evidence.
   - Grade 2: A source location relevant to the claim.
   - Grade 3: Evidence that demonstrates a failure path is excluded by the code.
   - Grade 4: Falsifiable execution of the real code with the relevant input and version that reaches the claimed code path. A substitute or an execution that misses the relevant branch does not qualify.
   - Grade 5: Reproduction in the running product with the relevant input, version, and code path.

   Grades 1 through 3 are static evidence and leave execution-dependent claims `unproven`. Report the attained grade and its scope. Do not extend evidence to inputs, versions, or paths it does not cover.
   A blocking finding names the violated requirement or contract, evidence, impact, and minimum fix. When an existing read-only command can prove the claim, record the command, expectation, and observation, and recheck it when cheap and reproducible. A planning gap may rest on a source contradiction or missing required evidence without new implementation. Pure style preferences, unreachable scenarios, and speculative improvements do not block. No confirmed finding is a valid result; missing required evidence does not pass.
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
