# Task Quality

Canonical questions for checking whether a task is sufficiently specified in its
`Change` and `Acceptance` text. The producer writing `## Tasks` answers every applicable question;
questions that do not apply are not mentioned.

## Questions

Before locking tasks, ground the plan in current behavior and answer these four
questions in the applicable task `Change` and `Acceptance` text. Cite the
existing file and symbol or behavior that supports each answer. If the code
does not establish an answer, record the gap and return the decision to
`/deep-planning` rather than filling it by assumption.

For a behavior task, the `Change` text also names the public entry that makes
the behavior observable. Prefer an existing public API. Name a new public
symbol only when the behavior requires a new API.

1. **Current-code basis:** What does the changed code do today? Cite every
   statement about existing behavior with its real `file:line`. Prove one
   concrete safety fact by running the
   relevant existing code when that is cheap. If it cannot be proven by a run,
   mark the fact `unproven` in the task. Do not present an unproven fact as
   established.
2. **Removal inventory:** Which existing guarantees, behaviors, or compatibility
   promises will the change remove or weaken? Cite their existing `file:line`
   basis, then state how the task keeps each guarantee or deliberately drops
   it.
3. **Execution reachability:** Can the named executor host, dispatched tester
   inputs, or named CI job produce the required evidence? Name the producer in
   both `Acceptance` and every covering `TP-NN` row. Route evidence that none
   can produce to `## Owner Acceptance` or `## Preconditions` before lock. Do
   not make acceptance depend on behavior introduced by a later task.
4. **Baseline-relative pass rule:** Which existing acceptance commands must run
   once on the base in read-only mode, what results must be recorded in
   `### Engineering Review`, and what is the pass rule relative to that
   baseline? If a suite already fails at the base, require no new failures
   against its recorded failure set.

These questions supplement the applicable questions below. Do not duplicate an
answer when the task text already makes it explicit. The task must still cite
the existing behavior and identify the evidence producer.

1. **Task-spec conformance:** What does this task change, and how does the change and acceptance text cover the task's stated scope?
2. **logical correctness:** What result must the change produce, and what evidence will show that the result is correct?
3. **Boundary conditions:** Which obvious local boundaries or edge cases does the task encounter, and what must happen at each one?
4. **Error handling:** Which failure path does this task introduce or affect, and what does it return or report?
5. **Return values and side effects:** What values, state changes, files, or external effects must the task produce, and which must it avoid?
6. **Layer boundaries:** Which architecture layer owns this change, and how does the task preserve the boundaries between layers?
7. **Existing-pattern consistency:** Which existing pattern or implementation does this task follow?
8. **Abstraction fit (YAGNI):** Why is the proposed abstraction needed, and why is a smaller or existing mechanism insufficient?
9. **Naming conventions:** Which naming rules apply, and what names must the task use?
10. **File and module organization:** Which files or modules change, and why does each belong in this task?
11. **Readability:** What structure, terminology, or explanation is required so the resulting change remains readable?
12. **duplicate code:** Which existing code could be duplicated by this task, and how will duplication be avoided or justified?
13. **dead code:** What does this task remove or update so that no obsolete code, path, or declaration is left unused?
14. **Performance:** Which obvious performance risk does this task introduce or affect, and what keeps it within acceptable bounds?

## Scope and Acceptance Traceability

Check coverage both ways against the original user intent, approved requirements,
and affected code. Every in-scope implementation behavior has an owning task
with behavior acceptance, and every task identifies its requirement or required
dependency. Every implementation task has at least one covering `TP-NN`.
Use task `Change` and `### Engineering Review` for these pointers, without a
permanent mapping table or second requirements list. Put human-only acceptance
in `## Owner Acceptance` and owner-only environment conditions in
`## Preconditions`. Neither creates an implementation task by itself.

Apply the finding criteria in [adversarial-review](../skills/adversarial-review/SKILL.md)
when reporting gaps.

## Shared-Contract Consumer Mapping

When any task producer plans a change to shared traits, CLI signatures, or
templates, or any of the following, require a **Blast-Radius Consumer Matrix** in the affected task
contract. Include direct and indirect consumers. For each consumer, identify
the file or interface, the change's impact, and either the owning task that
updates it or evidence that compatibility is preserved. Do not require a matrix
when none of these shared contracts changes. The additional triggers are:

- adding or changing a struct field without `Default`, or one used in a literal construction
- adding an enum variant
- changing a public function signature
- adding a required trait method

For each consumer, add columns for its safety premise, proof grade, and evidence
alongside the impact and owning task or compatibility evidence. Use
[blast-radius](../skills/blast-radius/SKILL.md) to assess the premise and grade
its evidence. Consumers sharing a premise may cite one execution with its scope
identified. Record missing execution evidence and its attained grade as
`unproven`; obtain proof or choose a design independent of that premise.
Existing task matrices remain valid under the contract in force when they were
written. These fields apply when a matrix is required, not to exempt tasks.

Include every affected consumer and dependency in the task allowlist or assign
it to its owning task. For each existing type, field, and function cited by task
`Change`, and each evidence field expected by `Acceptance` or a `TP-NN`, verify
existence in current code with read-only commands such as `rg`. An absent item
must be created by a named earlier task, or it is a plan gap. Record each
command and result in `### Engineering Review`. This applies to every task
producer, not only `/refining-plan`. Do not create a permanent hand-maintained
consumer registry.

## Comments in Changed Content

Apply the no-comments rule only to added or changed content. Use a comment only
for a legal notice, public API docs, a safety invariant, external compatibility,
or a necessary tool directive. Keep the reason for an exception clear in the
comment or its nearby documentation. Do not sweep unchanged legacy comments as
part of this rule.

Use encode-then-delete order: first express an encodable constraint as a type,
an assertion, or a test. Then remove only prose made redundant by that
constraint. Retain safety knowledge and conditions needed to apply the rule. A
new suppression directive must state its reason and receive audit. Do not
broaden existing suppressions.

For duplicate and dead-code decisions, `trim_guard` and `verify_delete` require location-backed evidence. Identify the guard behavior and its conditions before trimming; verify direct references at call sites and dynamic references through the mechanism that reaches the candidate. String matches alone neither establish reachability nor justify deletion. Confirm that any replacement preserves equivalent guard coverage for the affected inputs and callers. Preserve a guard's `when` condition unless evidence supports upgrading it to unconditional behavior. When reachability or equivalence is uncertain, defer the trim or deletion and record `upgrade when <condition>` with an observable trigger for reconsidering it. A bounded MUST KILL proposal may identify a removal candidate, but it does not override these safeguards. If a reshaping proposal falls outside the task allowlist, return it to the source plan. Any restructuring must remain bounded and preserve the demonstrated behavior.

## Consumers

- **Any producer that writes `## Tasks`:** Writes the answers to the applicable questions into each `T-NN` task's `Change` and `Acceptance` text. Questions that do not apply are not mentioned.
- **Any task producer:** Applies Shared-Contract Consumer Mapping, including its triggers, allowlist, and existence checks.
- **ORCHESTRATOR step 2b:** Reads all four grounding questions and the fourteen task-spec questions in this convention, then verifies the task text before writing the cursor. If an applicable question is unanswered, it prints the missing items, instructs the user to rerun `/refining-plan`, and stops without writing the cursor or any other file.
