# State Template

Template for `<repo>/.dev/state.md` — cross-session memory for per-repo work.

## File Template

```markdown
# Project State

## Current Position

Workflow: [IDLE | PLAN | DISCUSS | APPROVE | IMPLEMENT | TEST | CROSS_REVIEW | VERIFY | DONE]
Plan: [path to current plan file, or "None"]
Step: [N of M in current plan]
Last activity: [YYYY-MM-DD] — [what happened]

## Active Context

### Current Plan Summary

[One-liner: what we're building right now]

### Decisions Made

| Date | Decision | Rationale |
|------|----------|-----------|
| YYYY-MM-DD | [Choice] | [Why] |

### Deviations from Plan

| Step | Plan Said | Actually Did | Why |
|------|-----------|-------------|-----|

## Review Status

### Test Results

Last run: [YYYY-MM-DD]
Passed: [N] / Failed: [N] / Skipped: [N]

### Review Findings

Blocking: [N] / Warning: [N] / Info: [N]
Unresolved: [list of blocking issue IDs]

### Verification

Verdict: [NOT_STARTED | GAPS_FOUND | VERIFIED]
Gaps: [list if any]

## Blockers

[Issues preventing progress — empty when unblocked]

## Session Continuity

Last session: [YYYY-MM-DD HH:MM]
Stopped at: [description of last completed action]
Next step: [what to do when resuming]
```

## Usage Rules

1. **Only one active plan** — state.md tracks the current focus
2. **Update after every state transition** — PLAN→IMPLEMENT, IMPLEMENT→TEST, etc.
3. **Keep it current** — stale state is worse than no state
4. **Don't duplicate the plan** — link to it, don't copy content here
5. **Record deviations immediately** — before you forget why
