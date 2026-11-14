---
name: blast-radius
description: Verify load-bearing safety premises and trace consumers across symbols, serialized formats, configuration, and dependency boundaries before choosing or reviewing a design.
---

# Blast Radius

Use this skill before designing around existing behavior or reviewing a change that depends on a safety premise. Describe the change, its affected boundaries, and the behavior the design assumes. If a premise remains `unproven`, obtain the missing proof or choose a design that does not depend on it.

## Verify safety premises

1. Select one or two load-bearing safety premises whose failure would change the design or review verdict. State the relevant inputs, version, code path, and expected result for each.
2. Use the five proof grades in [adversarial-review](../adversarial-review/SKILL.md): assertion, source location, demonstrated exclusion of a failure path, falsifiable execution of real code, and reproduction in the running product. Match the evidence to the premise's inputs, version, and code path. Grades 1 through 3 are static evidence and leave the premise `unproven`.
3. Run the cheapest effective check that reaches grade 4 or higher. Record the command, expected result, observed result, attained grade, and scope. A stub or execution that misses the relevant branch does not establish grade 4. Multiple consumers may cite the same run when its scope covers each consumer.
4. If execution is unavailable or does not reach the relevant behavior, record the attained grade and the evidence gap and keep the premise `unproven`. Never treat missing execution as a pass. If execution contradicts the premise, record a confirmed risk and revise the design that depends on it.

## Trace consumers

Trace direct and indirect consumers from their read, write, load, or dispatch entry points to the affected behavior. Inspect serialized formats such as JSON and receipts, cross-language byte readers, configuration flags, and pinned or locally patched dependency source code as applicable. Check how each boundary is reached and what data or behavior it consumes. An empty symbol search does not exclude a consumer or clear its risk. Record unresolved reachability and compatibility gaps as `unproven`.

## Report

Report the change and each selected safety premise with its evidence, proof grade, and scope. Separate:

- **Confirmed risks:** execution results that contradict a premise, with the affected design and required revision.
- **Cleared risks:** compatibility supported by execution, with the inputs, version, code path, and consumers covered. Do not extend the clearance beyond that scope.
- **Unresolved risks:** `unproven` premises, consumers, or compatibility claims, with the attained grade and missing evidence.

For each unresolved or confirmed risk, name the cheapest effective pre-merge check, its expected result, and who can run it when known. If a check cannot be run, state the blocker and retain the risk.
