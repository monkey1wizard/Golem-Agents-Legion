---
name: gal-status
description: "GAL — full state projection. Shows active plans, workflow position, review and test results, blockers, session continuity, and specialist readiness."
---

# /gal status

Project the full recorded GAL state for this repo.

## Step 1 — Collect State

Starting from the current working directory or opened workspace folder, walk upward to the nearest ancestor directory that contains `.dev/state.md`. Treat that ancestor as the repo root and read `.dev/state.md` there.

- If no ancestor directory contains `.dev/state.md`, output **Repo not initialized — run `/gal init`.**
- If `.dev/state.md` exists but there is no active plan entry under `## Active Plans`, output **Repo is initialized but no active plan. Use `/planning` to start sprint planning.**
- If `.dev/state.md` exists and names an active plan, read that plan's execution file from the `File` column. Resolve markdown-wrapped relative paths against the current repo root. If the row points to `docs/plans/<slug>.md`, prefer `.dev/plans/<slug>.prompt.md` when it exists.
- If the active plan file is missing, output the exact repo-state error and suggest inspecting `.dev/state.md` plus the referenced active plan file.
- Also inspect `graphify-out/GRAPH_REPORT.md` and the optional `graphify-out/GAL_GRAPHIFY_VERSION.txt` when they exist. If the `graphify` CLI is available, capture `graphify --version` and classify graphify freshness as one of: `NOT-PRESENT`, `FRESH`, `STALE-BY-TOOL-VERSION`, or `UNSTAMPED`. Treat a report as stale only when the stamped version differs from the current version and `GRAPH_REPORT.md` is not newer than the stamp file.

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
- **Staff Review**: `CLEAR` / `FINDINGS-OPEN` / Pending from `<!-- STAFF_REVIEW: ... -->`
- **Design Review (Live)**: `CLEAR` / `FINDINGS-OPEN` / Pending from `<!-- DESIGN_REVIEW_LIVE: ... -->`
- **Security Review**: `CLEAR` / `FINDINGS-OPEN` / Pending from `<!-- SECURITY_REVIEW: ... -->`
- **Review Results**: any BLOCKING findings called out in `## Review Results`

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

### Graphify Freshness

From `graphify-out/GRAPH_REPORT.md`, the optional `graphify-out/GAL_GRAPHIFY_VERSION.txt`, and the current `graphify --version` output when available:

- **State**: `NOT-PRESENT` / `FRESH` / `STALE-BY-TOOL-VERSION` / `UNSTAMPED`
- **Report stamp**: stamped graphify version if the version file exists; otherwise *Not stamped*
- **Installed version**: current graphify version if available; otherwise *Unavailable*
- **Action**: if stale, tell the user to rerun `/graphify .` before the next graph-aware planning or review pass; otherwise say whether no action is required or freshness cannot be verified automatically

### Specialist Readiness

Based on the current plan files and progress markers, list the commands that are appropriate to invoke next:

| Signal | Appropriate Commands |
| --- | --- |
| Draft source plan, no execution prompt yet | `/deep-planning`, `/refining-plan` |
| Execution prompt exists, no eng review yet | Business review lane via configured provider or `/gal golem-analyst`; design review lane via configured provider or `/gal golem-designer`; engineering review lane via configured provider or `/refining-plan`, then refresh with `/plan-to-prompt` |
| Tasks initialized, work remaining | `/gal pipeline`, `golem-reviewer`, `golem-debugger`, `golem-designer` |
| Review-stage audit for customer-facing UI work with `DESIGN.md` in place | `golem-designer` in `audit` mode |
| Security-sensitive work touching auth, data handling, input handling, or public API surface | `golem-security` |
| Review clean, QA not yet run | `golem-tester` |
| Blocking review findings or failed tests | `golem-debugger`, return to implementation, then `golem-reviewer` |
| High-risk work on production systems, live data, or shared risky config | explicit user confirmation plus the relevant owning agent |
| Ready to hand off or pause | `/gal whats-next`, `/gal wrap-up`, `golem-releaser` |
