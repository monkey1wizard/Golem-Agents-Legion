---
name: gal-status
description: "GAL — full state projection. Shows active plans, workflow position, review and test results, blockers, session continuity, and specialist readiness."
---

# /gal status

Project the full recorded GAL state for this repo.

## Step 1 — Collect State

Starting from the current working directory or opened workspace folder, walk upward to the nearest ancestor directory that contains `.dev/state.md`. Treat that ancestor as the repo root and read `.dev/state.md` there.

- If no ancestor directory contains `.dev/state.md`, output **Repo not initialized — run `/gal init`.**
- If `.dev/state.md` exists but there is no active plan entry under `## Active Plans`, output **Repo is initialized but no active plan. Use `/office-hours` to start sprint planning.**
- If `.dev/state.md` exists and names an active plan, read that plan's execution file from the `File` column. Resolve markdown-wrapped relative paths against the current repo root. If the row points to `docs/plans/<slug>.md`, prefer `docs/plans/<slug>.prompt.md` when it exists.
- If the active plan file is missing, output the exact repo-state error and suggest inspecting `.dev/state.md` plus the referenced active plan file.

## Step 2 — Project

Using the file contents from `.dev/state.md` and the active plan file, produce the following sections in order:

### Active Plans

For each row in `.dev/state.md` Active Plans table:

- Plan name and file path
- Plan phase marker and current step
- Last activity date

### Current Position

From the primary active plan:

- Plan phase marker from `## Status > Workflow` if present; otherwise show *Not set*
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

Based on the current plan artifacts and progress markers, list the commands that are appropriate to invoke next:

| Signal | Appropriate Commands |
| --- | --- |
| Draft plan, no eng review yet | `/plan-eng-review`, `/plan-ceo-review`, `/autoplan`, `/office-hours` |
| Tasks initialized, work remaining | `/gal pipeline`, `/review`, `/investigate`, `/careful`, `/design-consultation` |
| Review clean, QA not yet run | `/qa`, `/qa-only` |
| Blocking review findings or failed tests | `/investigate`, return to implementation, then `/review` |
| Ready to hand off or pause | `/gal whats-next`, `/gal wrap-up`, `/learn`, `/browse` |
