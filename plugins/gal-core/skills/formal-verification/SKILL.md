---
name: formal-verification
description: Select and verify a high-risk state machine with bounded exploration, executable properties, negative checker tests, and replayable counterexamples.
---

# Formal Verification

Use bounded state-machine exploration when failures depend on sequences of transitions that handpicked tests may miss. The result covers only the stated initial conditions, events, and bound. It is not a proof of behavior outside that scope.

## Select the target and tool

1. Compare candidate state machines in the code. Identify their states, transition boundaries, failure costs, and sequence combinations that existing tests are unlikely to cover. Prioritize a bounded target where a violation has a high cost and sequence coverage adds value. Record why it was chosen over the other candidates.
2. Choose a tool that can exercise the relevant boundary. An in-language explorer can call real methods and reuse production types, but its finite inputs and depth limit coverage. A model checker can explore an abstract model and check richer properties, but model fidelity and integration with real code need separate evidence. Compare setup, runtime, state-space size, and replay support before choosing. State the trade-offs and excluded behavior.
3. Define initial conditions, eligible events and arguments, exploration order, and depth or other bounds. Invoke real transitions when the selected boundary permits it. Check each transition and retain the actual result and state for subsequent steps. Report the explored scope and any limits.

## Define and check properties

Give each property a stable ID. State the required behavior in one sentence and the consequence of a violation in another. Derive expected outcomes from the pre-transition state and relevant history, then compare them with the actual result and post-transition state. Include rejection paths and unchanged-state requirements where applicable.

For every property, automate a negative unit test that feeds a synthetic violating transition to the checker. Require a counterexample for the corresponding property with enough information to replay it. Also test valid transitions so the checker does not reject every input.

## Report and replay violations

On a real transition violation, fail the test and print a structured counterexample containing the property ID, initial conditions, ordered real methods and arguments, expected outcome, and actual outcome. Include setup and relevant history needed to reproduce the failure. Replay the sequence against the same code boundary and compare the observed result with the report. Investigate and fix the cause, then rerun the exploration and checker tests. Never weaken a property or count a violation as a pass to make the test succeed.
