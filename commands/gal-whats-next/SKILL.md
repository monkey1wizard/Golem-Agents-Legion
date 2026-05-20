---
name: gal-whats-next
description: "GAL — what to do next. Reads current plan state, review and test results, and recommends the single next specialist command or control-plane action."
---

# /gal whats-next

Determine what to do right now based on the current recorded GAL state.

## Step 1 — Collect State

Starting from the current working directory or opened workspace folder, walk upward to the nearest ancestor directory that contains `.dev/state.md`. Treat that ancestor as the repo root and read `.dev/state.md` there.

- If no ancestor directory contains `.dev/state.md`, output **Repo not initialized — run `/gal init`.**
- If `.dev/state.md` exists but there is no active plan entry under `## Active Plans`, output **No active plan. Use `/planning` to start sprint planning.**
- If `.dev/state.md` exists and names an active plan, read that plan's execution file from the `File` column. Resolve markdown-wrapped relative paths against the current repo root. If the row points to `docs/plans/<slug>.md`, prefer `.dev/plans/<slug>.prompt.md` when it exists.
- If the active plan file is missing, output the exact repo-state error and suggest inspecting `.dev/state.md` plus the referenced active plan file.

From `.dev/state.md` and the active plan file, extract these data points:

1. `.dev/state.md` — active plans table, blockers, session continuity
2. Active plan `## Status` — plan phase marker if present, current step, next step
3. Active plan `## Review Results` — any BLOCKING findings
4. Active plan `## Test Results` — pass / fail / pending
5. Active plan `### Handoff Notes` — interrupted work context
6. Active plan `## Open Questions` — count of unresolved OQ-NNN items
7. Active plan `## Tasks` — blocking task completion state
8. Active plan `## Deferred Follow-up` — advisory non-blocking work when present
9. Active plan `## Analyze` — CLEAR / DRIFT-OPEN / NOT-RUN verdict
10. Whether the active plan scope touches authentication, data storage, input handling, or public API surface
11. Graphify freshness from `graphify-out/GRAPH_REPORT.md`, the optional `graphify-out/GAL_GRAPHIFY_VERSION.txt`, and current `graphify --version` when available. Classify as `NOT-PRESENT`, `FRESH`, `STALE-BY-TOOL-VERSION`, or `UNSTAMPED`; only mark it stale when the stamped version differs and `GRAPH_REPORT.md` is not newer than the stamp file. Treat graphify as advisory context only, never as the gating next action for normal GAL flow.

## Step 2 — Decide

Apply this decision tree in order:

| Condition | Next Action |
| --- | --- |
| No active plan, no work in progress | Repo is already initialized; use `/planning` to start sprint planning |
| Active plan points to source plan only, no execution prompt yet | Run `/refining-plan` to lock the implementation contract into the source plan |
| No eng review recorded | Run the engineering review lane for the source plan through the configured provider, or use `/refining-plan` as the fallback, then refresh the prompt with `/plan-to-prompt` |
| Plan reviewed, tasks exist, implementation not started | Describe the first implementation task from the plan |
| `### Handoff Notes` contains an OPEN `Interrupted Phase` block | Resume that exact task and phase through `/gal pipeline`; do not start a new task |
| Implementation in progress, `### Handoff Notes` present | Resume from the exact "next step" in Handoff Notes |
| Implementation complete, no test results | `golem-tester` in `browser-qa` or `spec` mode, depending on the missing verification surface |
| Tests failing | Return to implementation — summarize what needs fixing |
| Tests passing, no review recorded | `golem-reviewer` for code review |
| Review has BLOCKING findings | Address the BLOCKING items — return to implementation |
| Review clean, security-sensitive scope, and no security review recorded | `golem-security` for a security audit before release work |
| `<!-- ANALYZE: DRIFT-OPEN -->` present | Code changes have drifted from plan scope — address deviations, then re-run `golem-reviewer` to update verdict |
| `## Tasks` has incomplete blocking items and no BLOCKING findings | Return to implementation — list remaining T-NNN tasks |
| All blocking `## Tasks` are complete and only `## Deferred Follow-up` remains | Continue toward verification, release prep, or wrap-up — do not reopen the implementation loop for advisory follow-up alone |
| Open OQs remain in `## Open Questions` | Note count as advisory — do not block; continue to next step |
| Review clean, plan not yet verified | `golem-releaser` for release prep, or `/gal wrap-up` if the user is pausing instead of landing |
| Blocker listed in `.dev/state.md` | State the blocker and what resolves it before any other action |
| Session continuity shows interrupted work | Resume from "Stopped at" in `.dev/state.md` `## Session Continuity` |

## Step 3 — Output

State in plain language:

1. **Where you are** — one sentence describing the current position in the plan lifecycle
2. **Next action** — the single command, review lane, or task to start
3. **Open this first** — which file or context is needed to begin
4. **Graphify note** — only when graphify freshness is `STALE-BY-TOOL-VERSION` or `UNSTAMPED`; if stale, say GAL can continue without graphify and the user may refresh graphify artifacts manually if they want updated graph context

Do not present multiple options. Commit to one clear next step.
