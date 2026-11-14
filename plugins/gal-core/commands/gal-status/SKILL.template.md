---
name: gal-status
description: "GAL status ($gal-status / /gal status / 狀態 / 現況 / what's the status / show status). Full state projection: active plans, workflow position (Current Task, Step), review/test results, blockers, session continuity, specialist readiness. Read from .dev/state.md + active .prompt.md."
---

# /gal status

Project the full recorded GAL state for this repo.

## Step 1 — Collect State

Starting from the current working directory or opened workspace folder, walk upward to the nearest ancestor directory that contains `.dev/state.md`. Treat that ancestor as the repo root and read `.dev/state.md` there.

- If no ancestor directory contains `.dev/state.md`, output **Repo not initialized — run `/gal init`.**
- If `.dev/state.md` exists but there is no active plan entry under `## Active Plans`, output **Repo is initialized but no active plan. Use `/planning` to start sprint planning.**
- Resolve every row in `## Active Plans`. Table order is priority order. Treat the **primary active plan** as the first row whose plan phase is not terminal (`Complete`, `Done`, `Verified`, `Closed`); if all rows are terminal, fall back to the first row.
- For each resolved row, read that plan's execution file from the `File` column. Resolve markdown-wrapped relative paths against the current repo root. If the row points to `.dev/plans/<slug>.md`, prefer `.dev/plans/<slug>.prompt.md` when it exists, but also keep the source plan path for source/prompt task-sync checks. If the row points directly to `.dev/plans/<slug>.prompt.md`, also resolve the paired `.dev/plans/<slug>.md` when it exists.
- If the active plan file is missing, output the exact repo-state error and suggest inspecting `.dev/state.md` plus the referenced active plan file.

## Step 2 — Project

Using the file contents from `.dev/state.md` and the active plan file, produce the following sections in order:

### Active Plans

For each row in `.dev/state.md` Active Plans table:

- Plan name and file path
- Plan phase marker and current step
- Last activity date
- Whether the row is the current primary active plan

### Current Position

From the primary active plan:

- Plan phase marker from `## Status > Workflow` if present; otherwise show *Not set*
- Current step and total steps
- What the plan's `## Status` says the next step is
- Current task from `## Status > Current Task`
- Task commit markers from `## Status > Task Base Commit` and `Task Final Commit`
- Any deviations recorded in the Deviations table
- Any execution write-back gap where the workflow implies progress but the durable markers are still missing or placeholder-only

### Review & Test Status

For each active plan with results filled in:

- **Test Results**: Pass / Fail / Pending — one-line summary from `## Test Results`
- **Staff Review**: `CLEAR` / `FINDINGS-OPEN` / Pending from `<!-- STAFF_REVIEW: ... -->`
- **Design Review (Live)**: `CLEAR` / `FINDINGS-OPEN` / Pending from `<!-- DESIGN_REVIEW_LIVE: ... -->`
- **Security Review**: `CLEAR` / `FINDINGS-OPEN` / Pending from `<!-- SECURITY_REVIEW: ... -->`
- **Review Results**: any BLOCKING findings called out in `## Review Results`

If sections are unpopulated placeholders, show: *Pending.*

### Spec Readiness

From `## Open Questions`, `## Tasks`, `## Deferred Follow-up` when present, and `## Analyze` in the active plan's `.prompt.md`:

- **Open Questions**: count of unresolved `OQ-NN` items (`- [ ]`)
- **Blocking Tasks**: X of Y complete (count checked vs total `T-NN` items under `## Tasks` only)
- **Source/prompt task sync**: `HEALTHY` when paired `.dev/plans/<slug>.md` and `.dev/plans/<slug>.prompt.md` agree on blocking `T-NN` checkbox state; otherwise `MISMATCH` with the task ids that disagree
- **Deferred Follow-up**: advisory count from `## Deferred Follow-up` when present; if missing, show *None recorded.*
- **Analyze verdict**: `CLEAR` / `DRIFT-OPEN` / `NOT-RUN` from `<!-- ANALYZE: ... -->`
- **Execution write-back health**: `HEALTHY` / `MISSING-DURABLE-STATE` based on whether `Workflow`, `Current Task`, `Next step`, task commit markers, task-scoped test/review results, the matching `.dev/state.md` session continuity row, and source/prompt task checkbox sync exist when the current phase implies they should

If any section is missing or not yet initialized: show *Not yet run.*

### Blockers

From `.dev/state.md` `## Blockers` section.
If the section is empty or not present: **None.**

### Session Continuity

From `.dev/state.md` `## Session Continuity`, using one row per active plan matched by paired source plan path:

- Plan name
- Source plan path
- Last session
- Stopped at
- Next step
- Active context
- If an active plan is missing a continuity row, show `MISSING` for that plan instead of reusing another plan's row

### Planning-Language Compatibility

Track the named source plan, its actual review results, and the current receipt for all languages.

### Specialist Readiness

Based on the current plan files and progress markers, list the commands that are appropriate to invoke next:

| Signal | Appropriate Commands |
| --- | --- |
| Draft source plan, no execution prompt yet | `/deep-planning` when architect review is needed; complete placeholder `## Tasks` or `## Test Plan` per `plugins/gal-core/templates/plan.md` with any producer (`/refining-plan` is one option) |
| Source plan exists, planning-stage domain reviews not yet run | `/deep-planning` when architect review is needed; business review lane via configured provider or `/gal analyst`; design review lane via configured provider or `/gal designer`; complete the STAGE 3.5 dual-lens review and pass `gal refining-check`, then record human approval in `## Approval` and use `/plan-to-prompt` |
| Tasks initialized, work remaining | `/gal pipeline`, `golem-auditor`, `golem-debugger`, `golem-designer` |
| Review-stage audit for customer-facing UI work with `DESIGN.md` in place | `golem-designer` in `audit` mode |
| Security-sensitive or deep-risk work touching auth, data handling, input handling, or public API surface | `golem-auditor` |
| Review clean, QA not yet run | `golem-tester` |
| Blocking audit findings or failed tests | `golem-debugger`, return to implementation, then `golem-auditor` |
| High-risk work on production systems, live data, or shared risky config | explicit user confirmation plus the relevant owning agent |
| Ready to hand off or pause | `/gal whats-next`, `/gal wrap-up`, `/gal finalize` |
