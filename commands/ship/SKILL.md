---
name: ship
description: "Release Engineer — final gate before merge. Syncs main, runs tests, fills coverage gaps, checks Review Readiness Dashboard, pushes branch, creates PR, and auto-invokes /document-release. The only required gate is Eng Review."
---

# /ship

Final mile before merge. Get the branch into a PR with passing tests.

## Role

Release engineer. Your job: clean branch, passing tests, PR open, docs current.

## When to Use

- After implementation, review, and QA
- When `/gal whats-next` says the branch is ready to ship
- Eng Review (`<!-- ENG_REVIEW: CLEAR -->`) is the only hard gate — ask if missing, don't hard-block

## Step 1 — Read Review Readiness Dashboard

Read the active plan's `## Review Results` section. Check:

| Gate | Status | Action |
| --- | --- | --- |
| `<!-- ENG_REVIEW: CLEAR -->` | Required | If missing: ask "Engineering review not found. Run the engineering review lane first, or proceed anyway?" |
| `<!-- QA: CLEAR -->` | Recommended | If missing: warn, continue unless user says stop |
| `<!-- STAFF_REVIEW: CLEAR -->` | Recommended | If missing: warn, continue unless user says stop |
| `<!-- DESIGN_REVIEW_LIVE: FINDINGS-OPEN -->` | Conditional | If the change touches customer-facing UI: warn and confirm before proceeding |
| `<!-- SECURITY_REVIEW: FINDINGS-OPEN -->` | Conditional | If the change touches auth, data, input handling, or public API surface: warn and confirm before proceeding |
| Open `## Open Questions` | Warning | Count unresolved `OQ-NNN` items — list them if any remain |
| Incomplete `## Tasks` | Warning | Count unchecked `T-NNN` items — list remaining tasks |
| `<!-- ANALYZE: DRIFT-OPEN -->` | Warning | If present: warn that diff has drifted from plan scope |

Design and security audits are conditional review-stage checks. Note their state, warn when they are missing for relevant scope, and do not treat them as universal hard gates.

## Step 2 — Sync Main

```
git fetch origin
git merge origin/main
```

If conflicts: stop and tell the user. Do not attempt auto-resolution of merge conflicts.

## Step 3 — Run Tests

Run the existing test suite. Record: total tests, passing, failing, new failures.

If the project has no test framework: bootstrap one.
1. Detect runtime (Node / Python / Go / Rust / other) from project files
2. Install the standard framework for that runtime
3. Write 3–5 real tests covering the core behavior implemented in this branch
4. Set up GitHub Actions CI if no `.github/workflows/` exists
5. Create `TESTING.md` with instructions for running tests locally
6. Commit: `test(bootstrap): add test framework and initial tests`

## Step 4 — Fill Coverage Gaps

Produce an ASCII coverage diagram with quality stars:

```
src/
  feature/
    handler.ts    ★★★★☆  (80%)
    service.ts    ★★☆☆☆  (40%) ← gap
    utils.ts      ★★★★★  (100%)
```

For any file with < 60% coverage that is directly related to this branch's changes: write additional tests. Commit: `test(coverage): improve coverage for <file>`

## Step 5 — Push Branch and Create PR

```
git push origin HEAD
```

Create or update the PR with:
- Title: conventional commit title matching the plan goal
- Body: `## What changed`, `## Test delta: N → M`, `## Coverage`, link to plan file
- Link any open `## Review Results` findings as checkboxes

## Step 6 — Auto-Invoke /document-release

Run `/document-release` automatically after PR is created.

## Step 7 — Triage Greptile Comments (if installed)

If Greptile PR comments are present: triage each as:
- Valid finding → fix and commit
- Already addressed → auto-reply with explanation
- False positive → reply with pushback and reasoning

## Step 8 — Write Back to Plan

In the active plan file, append:

```markdown
## Ship

**Date:** <today>
**PR:** <URL>
**Tests:** <before N> → <after M>
**Coverage:** <summary>
**Greptile triage:** <N valid / M FP / K already fixed>
```

Tell the user: PR URL, test delta, any open warnings from Review Readiness.

Suggest: `/land-and-deploy` when PR is approved and CI passes.
