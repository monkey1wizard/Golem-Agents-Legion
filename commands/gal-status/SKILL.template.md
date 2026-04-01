---
name: gal-status
description: "GAL — full state projection. Shows active plans, workflow position, review and test results, blockers, session continuity, and specialist readiness."
---

# /gal status

Project the full recorded GAL state for this repo.

## Step 1 — Read

Read these files:

1. `.dev/state.md` — active plans, global decisions, blockers, session continuity
2. Each plan file listed in the Active Plans table — `## Status`, `## Review Results`, `## Test Results`, `## Open Questions`, `## Tasks`, `## Analyze`, `### Handoff Notes`

If `.dev/state.md` does not exist: output **Repo not initialized — run `/gal init`.**

## Step 2 — Project

Output the following sections in order:

### Active Plans

For each row in `.dev/state.md` Active Plans table:
- Plan name and file path
- Workflow state and current step
- Last activity date

### Current Position

From the primary active plan:
- Workflow state (DRAFT / PLAN / IMPLEMENT / TEST / REVIEW / VERIFY / DONE)
- Current step and total steps
- What the plan's `## Status` says the next step is
- Any deviations recorded in the Deviations table

### Review & Test Status

For each active plan with results filled in:
- **Test Results**: Pass / Fail / Pending — one-line summary from `## Test Results`
- **Review Results**: verdict and any BLOCKING findings from `## Review Results`

If sections are unpopulated placeholders, show: *Pending.*

### Spec Readiness

From `## Open Questions`, `## Tasks`, and `## Analyze` in the active plan's `.prompt.md`:

- **Open Questions**: count of unresolved `OQ-NNN` items (`- [ ]`)
- **Tasks**: X of Y complete (count checked vs total `T-NNN` items)
- **Analyze verdict**: `CLEAR` / `DRIFT-OPEN` / `NOT-RUN` from `<!-- ANALYZE: ... -->`

If any section is missing or not yet initialized: show *Not yet run.*

### Blockers

From `.dev/state.md` `## Blockers` section.
If the section is empty or not present: **None.**

### Session Continuity

From `.dev/state.md` `## Session Continuity`:
- Last session
- Stopped at
- Next step
- Active context

### Specialist Readiness

Based on the current workflow state, list the commands that are appropriate to invoke next:

| Workflow State | Appropriate Commands |
| --- | --- |
| DRAFT / PLAN | `/plan-eng-review`, `/plan-ceo-review`, `/autoplan`, `/office-hours` |
| IMPLEMENT | `/review`, `/investigate`, `/careful`, `/design-consultation` |
| TEST | `/qa`, `/qa-only` |
| REVIEW | `/review`, `/design-review`, `/investigate` |
| Any | `/gal whats-next`, `/gal wrap-up`, `/learn`, `/browse` |
