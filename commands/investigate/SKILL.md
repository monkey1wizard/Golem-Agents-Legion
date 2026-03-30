---
name: investigate
description: "Systematic debugger. Traces data flow, tests one hypothesis at a time, and requires root-cause confirmation before any fix. Auto-activates /freeze on the module being debugged."
---

# /investigate

Debug a problem systematically. No fixes without a confirmed root cause.

## Role

Systematic debugger. Your iron law: **diagnose before you fix**.

## When to Use

- Any time a bug is reported
- When tests are failing and the cause is not obvious
- When `/review` flags a potential bug that needs investigation before a fix

## Iron Law

Do not write a fix until you have confirmed the root cause. If you cannot confirm the root cause after 3 hypothesis cycles, stop and re-examine the architecture.

## Step 1 — Read Context

Read the active plan file from `.dev/state.md`. Note: what was recently implemented, what changed, what is failing.

Ask the user: "Describe the symptom — what happens, what was expected, and when it started."

## Step 2 — Auto-Freeze the Module

Before reading any code: identify the primary module or directory under investigation and activate freeze mode.

State to the user: "I'm freezing edits outside `<module-path>` for this investigation to prevent accidental changes while debugging."

Freeze scope: do not make edits outside the identified module until investigation is complete.

## Step 3 — Trace Data Flow

Trace the execution path relevant to the reported symptom:

1. Entry point (where does the problematic operation start?)
2. Data transformations (what changes at each layer?)
3. Output (where should it land, and where does it actually land?)
4. Side effects (what else fires along this path?)

Document the trace before forming hypotheses.

## Step 4 — Form Hypotheses

List the 3 most likely root causes based on the trace. Rank by probability.

For each hypothesis:
- State what would be true if this is the cause
- State what evidence would confirm or deny it
- State how to test it with the smallest possible change

## Step 5 — Test One Hypothesis at a Time

Test the highest-probability hypothesis first:

- Add a targeted log, assertion, or minimal code change to confirm or deny it
- Run the relevant test or reproduce the bug
- Record the result: CONFIRMED / DENIED
- If DENIED: move to the next hypothesis

Do not test multiple hypotheses simultaneously.

**If 3 hypotheses are all denied:** pause. Revisit the trace. Consider whether the bug is at the boundary between modules rather than inside one.

## Step 6 — Confirm Root Cause

Before writing any fix, state:
- "Root cause confirmed: [description]"
- The exact file and line where the bug originates
- Why this caused the reported symptom

Do not proceed to Step 7 without this confirmation.

## Step 7 — Write the Fix

Write the minimal fix that addresses the root cause. Do not fix anything else in the same commit.

Commit: `fix(<module>): <root cause description>`

## Step 8 — Verify

Re-run the test or reproduction steps. Confirm the symptom is gone.

If the fix introduced a regression: return to Step 3 for the new symptom.

## Step 9 — Write Back to Plan

In the active plan file, append:

```markdown
## Debug Session

**Date:** <today>
**Symptom:** <what was reported>

#### Hypothesis Chain

| # | Hypothesis | Result |
| --- | --- | --- |
| 1 | ... | DENIED |
| 2 | ... | CONFIRMED |

#### Root Cause

<File:line — description of the confirmed root cause.>

#### Fix Applied

Commit: <hash> — <description>

#### Verified

<Yes / Partial / No — and what still needs attention.>
```

Tell the user: root cause, fix applied, whether `/review` should be run if the fix was substantial.
