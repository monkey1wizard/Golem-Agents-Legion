---
name: qa-only
description: "Pure QA bug report mode — same testing as /qa but no fix loop. Records every bug found in ## Test Results (Report Only) and stops. Use when you want a second opinion or want to fix bugs manually."
---

# /qa-only

Find all bugs. Do not fix anything. Write a complete bug report.

## Role

QA auditor. Your job: accurate, complete, reproducible bug reports — not fixes.

## When to Use

- When you want a full audit before deciding what to fix
- When someone else (human or agent) will do the fixing
- When `/review` feedback is "test first, fix second"
- As a second-opinion pass after `/qa` on a critical branch

## The Only Difference from /qa

`/qa-only` runs all the same test steps as `/qa` but **does not attempt any fixes**.

If you catch yourself about to write code: stop. Log the bug and move to the next scenario.

## Step 1 — Read the Test Plan

Read the active plan file from `.dev/state.md`. Find `## Test Plan`.

If no `## Test Plan` exists: tell the user "No Test Plan found. I will test based on the plan requirements and visible UI instead."

## Step 2 — Start a Browser Session

Use `/browse goto <dev-url>`. If authentication is needed: prompt the user to run `/setup-browser-cookies` first.

## Step 3 — Run All Test Scenarios

For each scenario in the Test Plan:

1. Use `/browse` to navigate and interact
2. Check expected vs actual result
3. Record: PASS / FAIL / BLOCKED (with reason, screenshot, and exact reproduction steps)

For FAIL: record a reproduction recipe with exact steps so any developer can reproduce it.

## Step 4 — Edge Case Sweep

Same as `/qa`:
- Empty states
- Form validation
- Network error states
- Mobile viewport (375px)

## Step 5 — Bug Severity Rating

Rate each FAIL:

| Severity | Criteria |
| --- | --- |
| P0 | Data loss, security failure, app unusable |
| P1 | Feature broken, no workaround |
| P2 | Feature broken, workaround exists |
| P3 | Visual / cosmetic defect |

## Step 6 — Write Back to Plan

In the active plan file, append:

```markdown
## Test Results (Report Only)

**Date:** <today>
**Health Score:** <0–100>

#### Bug Report

| # | Severity | Scenario | Description | Repro Steps |
| --- | --- | --- | --- | --- |
| BUG-001 | P1 | ... | ... | 1. Go to X. 2. Click Y. 3. Expected Z. Actual W. |

#### Passed Scenarios (<N> of <Total>)

<List passing scenarios.>

<!-- QA_ONLY: REPORT-COMPLETE -->
```

Save a full report to `docs/qa-reports/<date>-<plan-slug>-report-only.md`.

Tell the user: health score, bug count by severity, and suggested next command (`/qa` to fix, or `/investigate` for deep bugs).
