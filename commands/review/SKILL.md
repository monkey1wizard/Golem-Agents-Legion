---
name: review
description: "Paranoid staff engineer review of the current branch changes. Finds N+1 queries, race conditions, trust boundary violations, forgotten enum handlers, and completeness gaps. Auto-fixes mechanical issues. Writes ## Staff Review to the active plan."
---

# /review

Review the current branch changes as a paranoid staff engineer. Find what CI misses.

## Role

Staff engineer. Your job is to find the bugs that pass all tests and only break in production.

This is the post-implementation review of code changes for the workflow review stage. It does not replace the planning-stage engineering review lane.

## When to Use

- After implementation, before `/ship`
- After a significant `/investigate` fix
- When `/gal whats-next` recommends review

## Step 1 — Review Changes

Run `git diff main` (or `git diff origin/main` if on a remote branch) and read the full set of changes.

Read the active plan file from `.dev/state.md` — note what was intended to be built.

Also read `## Tasks` and `## Open Questions` from the plan's `.prompt.md` — use the task list to check completeness against the changes.

## Step 2 — Bug Pattern Scan

Check for each pattern. Auto-fix if mechanical and obvious. Flag for user decision if ambiguous.

### Database & Query Bugs
- **N+1 queries** — loop that issues a query per iteration; replace with batch load
- **Stale reads** — reading data that was just written without a refresh/invalidation
- **Missing indexes** — new query on a column with no index; flag for addition
- **Race conditions** — parallel writes to the same row without a lock or transaction

### Trust & Security Bugs
- **Bad trust boundaries** — user-controlled input used in a privileged context without sanitization
- **Escaping bugs** — HTML, SQL, or shell injection vectors
- **Auth check placement** — authorization check after data lookup instead of before

### Logic Bugs
- **Broken invariants** — a data constraint the rest of the code assumes, not enforced here
- **Forgotten enum handlers** — a new enum constant added but not handled in every switch/allowlist that reads it; trace the constant through all callsites
- **Bad retry logic** — retrying non-idempotent operations; retrying without backoff; not retrying retriable errors

### Completeness Gaps
- **Feature completeness** — a requirement from the plan that is partially implemented; if the full solution costs < 30 minutes, complete it
- **Edge case coverage** — empty collection, zero, null, boundary values — are these handled or silently wrong?
- **CI-passes-but-breaks-in-prod** — anything that works locally or in tests but will fail under real load, real data, or a different environment

## Step 3 — Auto-Fix Mechanical Issues

For each finding that is:
- Unambiguously wrong (not a style or taste decision)
- The fix has a single correct solution
- The fix touches fewer than 20 lines

Fix it directly and commit: `fix(review): <description>`

## Step 4 — Flag Ambiguous Findings

For each finding that requires judgment:
- State the finding with file:line evidence
- Explain why it matters
- Propose a specific fix
- Ask the user: "Fix as proposed? Or describe your preferred approach."

## Step 5 — Write Back to Plan

In the active plan file, append under `## Review Results`:

```markdown
### Staff Review

**Date:** <today>
**Changes reviewed:** <branch> vs main

#### Auto-Fixed (<N> items)

| Finding | File | Fix | Commit |
| --- | --- | --- | --- |

#### Flagged for Decision (<N> items)

| Finding | Severity | Description |
| --- | --- | --- |

#### Verdict

<!-- STAFF_REVIEW: CLEAR -->
```

Replace `CLEAR` with `FINDINGS-OPEN` if there are flagged items the user has not yet resolved.

**Write `## Analyze`:**

After reviewing the changes against the plan, overwrite the `## Analyze` section of the `.prompt.md` with the verdict:

```markdown
## Analyze

**Date:** <today>
**Branch changes:** <branch> vs main

| Check | Result |
| --- | --- |
| All T-NNN tasks addressed by the changes | ✓ / ✗ — <count> of <total> complete |
| Changes stay within plan scope | ✓ / ✗ — <note any unplanned work> |
| Requirements vs implementation | ✓ / ✗ — <gaps if any> |

<!-- ANALYZE: CLEAR -->
```

Replace `CLEAR` with `DRIFT-OPEN` if any task is uncomplete or the changes include significant out-of-scope work. Replace with `NOT-RUN` only if the plan has no `## Tasks` and no requirements to check against.

Tell the user: auto-fixed count, open findings count, whether the branch is ready for `/ship`, and the `ANALYZE` verdict.
