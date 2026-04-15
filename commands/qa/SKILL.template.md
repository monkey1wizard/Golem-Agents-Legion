---
name: qa
description: "Full QA lead mode. Reads the ## Test Plan from the active plan, tests each scenario in real Chromium via /browse, fixes every bug found, writes regression tests, and records results in ## Test Results with a health score."
---

# /qa

QA lead mode. Find every bug in the Test Plan, fix it, write regression tests.

## Role

QA lead. Your job: zero known bugs before `/ship`.

## When to Use

- After implementation, before `/ship`
- When `## Test Plan` exists in the active plan
- When `/gal whats-next` recommends QA

## Modes

| Flag | Behavior |
| --- | --- |
| _(default)_ | Diff-aware: test only scenarios related to recent changes |
| `--full` | Test all scenarios in `## Test Plan` |
| `--quick` | Smoke tests only (happy paths, 5-minute budget) |
| `--regression baseline.json` | Compare against a previous `/qa` report |

## Step 1 — Read the Test Plan

Read the active plan file from `.dev/state.md`. Find `## Test Plan`.

If no `## Test Plan` exists: tell the user "No Test Plan found. Run the engineering review lane to generate one, or describe what to test and I will create a Test Plan now."

## Step 2 — Start a Browser Session

Use `/browse goto <dev-url>`. If authentication is needed: prompt the user to run `/setup-browser-cookies` first.

## Step 3 — Run Test Scenarios

For each scenario in the Test Plan (or diff-aware subset in default mode):

1. Use `/browse` to navigate and interact
2. Check expected vs actual result
3. Record: PASS / FAIL / BLOCKED (with reason)

If FAIL: proceed to Step 4 (fix loop). If BLOCKED: log and continue.

## Step 4 — Fix Loop (per failing scenario)

For each FAIL:

1. Locate the source — trace from the UI failure back to the root file
2. Write the minimal fix
3. Commit: `fix(qa): <scenario> — <description>`
4. Re-run the scenario in the browser: confirm PASS
5. Write a regression test that would have caught this bug
6. Add the regression test to the next commit: `test(qa): regression for <scenario>`

Do not accumulate multiple fixes in a single commit.

## Step 5 — Edge Case Sweep

After all planned scenarios pass, check:
- Empty states (no data, no results)
- Form validation (required fields, invalid formats)
- Network error states (simulate with DevTools throttle if available)
- Mobile viewport (resize to 375px width, recheck key flows)

Log PASS / FAIL for each.

## Step 6 — Calculate Health Score

```
Health Score = (PASS / (PASS + FAIL + BLOCKED)) × 100
```

Adjust down by 5 for each BLOCKED scenario (unknown risk).

## Step 7 — Write Back to Plan

In the active plan file, append:

```markdown
## Test Results

**Date:** <today>
**Mode:** <default | full | quick | regression>
**Health Score:** <0–100>

#### Scenario Results

| Scenario | Result | Notes |
| --- | --- | --- |
| ... | PASS | |
| ... | FAIL→FIXED | fix commit: abc1234 |

#### Regression Tests Added

- `test(qa): <description>` — <file>

#### Open Issues (<N> items)

| # | Severity | Description | Status |
| --- | --- | --- | --- |

<!-- QA: CLEAR -->
```

Replace `CLEAR` with `FINDINGS-OPEN` if there are unresolved FAIL or BLOCKED items.

Save a full report to `docs/qa-reports/YYYYMMDD-<plan-slug>.md` where `YYYYMMDD` is today's date (e.g. `20260401-auth-refresh.md`).

Tell the user: health score, fixed count, open issues count, and whether the branch is ready for `/ship`.
