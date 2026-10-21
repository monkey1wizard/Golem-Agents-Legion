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
- Also inspect `graphify-out/GRAPH_REPORT.md` and the optional `graphify-out/GAL_GRAPHIFY_VERSION.txt` when they exist. If the `graphify` CLI is available, capture `graphify --version` and classify graphify freshness as one of: `NOT-PRESENT`, `FRESH`, `STALE-BY-TOOL-VERSION`, or `UNSTAMPED`. Treat a report as `FRESH` whenever `GRAPH_REPORT.md` exists and GAL cannot prove a stale-by-tool-version mismatch. Treat a report as stale only when the stamped version differs from the current version and `GRAPH_REPORT.md` is not newer than the stamp file. If the report is usable but no version stamp exists, keep the state `FRESH`, show `Report stamp: Not stamped`, and note that automatic version verification is unavailable.

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

### Planning-Language Draft Health

If a localized source plan carries a planning-authority metadata block (`planLanguage != en`, the EN-draft flow — see `workflows/coding.md`), report its **draft health**: whether the EN semantic draft (`.dev/plans/<slug>.en.md`) exists and whether the localized rendered-source hash matches the metadata (a mismatch means a pending reconcile). **Never treat the EN draft as an active plan** — it is pre-prompt transient authority, not a `.dev/plans/` source plan, and must not appear in the Active Plans projection or be offered as a `/gal pipeline` / `/gal finalize` target.

### Graphify Freshness

From `graphify-out/GRAPH_REPORT.md`, the optional `graphify-out/GAL_GRAPHIFY_VERSION.txt`, and the current `graphify --version` output when available:

- **State**: `NOT-PRESENT` / `FRESH` / `STALE-BY-TOOL-VERSION` / `UNSTAMPED`
- **Report stamp**: stamped graphify version if the version file exists; otherwise *Not stamped*
- **Installed version**: current graphify version if available; otherwise *Unavailable*
- **Action**: if stale, note that GAL can continue without graphify and the user may refresh graphify artifacts manually if they want updated graph context; if the report is missing, say that no action is required for normal GAL flow; if the report exists without a version stamp, say that no action is required for normal GAL flow and that automatic version verification is unavailable; otherwise say whether no action is required or freshness cannot be verified automatically

### Specialist Readiness

Based on the current plan files and progress markers, list the commands that are appropriate to invoke next:

| Signal | Appropriate Commands |
| --- | --- |
| Draft source plan, no execution prompt yet | `/deep-planning`, `/refining-plan` |
| Source plan exists, planning-stage domain reviews not yet run | `/deep-planning`; business review lane via configured provider or `/gal analyst`; design review lane via configured provider or `/gal designer`; if engineering review contract is still uninitialized, run `/refining-plan`, then record human approval in `## Approval`, then refresh with `/plan-to-prompt` |
| Tasks initialized, work remaining | `/gal pipeline`, `golem-auditor`, `golem-debugger`, `golem-designer` |
| Review-stage audit for customer-facing UI work with `DESIGN.md` in place | `golem-designer` in `audit` mode |
| Security-sensitive or deep-risk work touching auth, data handling, input handling, or public API surface | `golem-auditor` |
| Review clean, QA not yet run | `golem-tester` |
| Blocking audit findings or failed tests | `golem-debugger`, return to implementation, then `golem-auditor` |
| High-risk work on production systems, live data, or shared risky config | explicit user confirmation plus the relevant owning agent |
| Ready to hand off or pause | `/gal whats-next`, `/gal wrap-up`, `/gal finalize` |
